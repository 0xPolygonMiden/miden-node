use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

use miden_objects::{
    accounts::AccountId,
    notes::{NoteId, Nullifier},
    transaction::{OutputNote, OutputNotes, ProvenTransaction, TransactionId},
    Digest,
};

use crate::{
    errors::{AddTransactionError, VerifyTxError},
    store::TransactionInputs,
};

mod account_state;

use account_state::InflightAccountState;

/// Tracks the inflight state of the mempool. This includes recently committed blocks.
///
/// Allows appending and reverting transactions as well as marking them
/// as part of a committed block. Committed state can also be pruned once the
/// state is considered past the stale threshold.
#[derive(Default, Debug, PartialEq)]
pub struct InflightState {
    /// Account states from inflight transactions.
    ///
    /// Accounts which are [AccountStatus::Empty] are immedietely pruned.
    accounts: BTreeMap<AccountId, InflightAccountState>,

    /// Nullifiers produced by the input notes of inflight transactions.
    nullifiers: BTreeSet<Nullifier>,

    /// Notes created by inflight transactions.
    ///
    /// Some of these may already be consumed - check the nullifiers.
    output_notes: BTreeMap<NoteId, OutputNoteState>,
}

/// The aggregated impact of a set of sequential transactions on the [InflightState].
pub struct StateDelta {
    /// The number of transactions that affected each account.
    account_transactions: BTreeMap<AccountId, usize>,

    /// The nullifiers consumed by the transactions.
    nullifiers: BTreeSet<Nullifier>,

    /// The notes produced by the transactions.
    output_notes: BTreeSet<NoteId>,
}

impl StateDelta {
    pub fn new(txs: &[ProvenTransaction]) -> Self {
        let mut account_transactions = BTreeMap::<AccountId, usize>::new();
        let mut nullifiers = BTreeSet::new();
        let mut output_notes = BTreeSet::new();

        for tx in txs {
            *account_transactions.entry(tx.account_id()).or_default() += 1;
            nullifiers.extend(tx.get_nullifiers());
            output_notes.extend(tx.output_notes().iter().map(|note| note.id()));
        }

        Self {
            account_transactions,
            nullifiers,
            output_notes,
        }
    }
}

impl InflightState {
    /// Appends the transaction to the inflight state.
    ///
    /// This operation is atomic i.e. a rejected transaction has no impact of the state.
    pub fn add_transaction(
        &mut self,
        tx: &ProvenTransaction,
        input_account_hash: Option<Digest>,
    ) -> Result<BTreeSet<TransactionId>, VerifyTxError> {
        // Separate verification and state mutation so that a rejected transaction
        // does not impact the state (atomicity).
        self.verify_transaction(tx, input_account_hash)?;

        let parents = self.insert_transaction(tx);

        Ok(parents)
    }

    fn verify_transaction(
        &self,
        tx: &ProvenTransaction,
        input_account_hash: Option<Digest>,
    ) -> Result<(), VerifyTxError> {
        // Ensure current account state is correct.
        let current = self
            .accounts
            .get(&tx.account_id())
            .and_then(|account_state| account_state.current_state())
            .copied()
            .or(input_account_hash);
        let expected = tx.account_update().init_state_hash();

        if expected != current.unwrap_or_default() {
            return Err(VerifyTxError::IncorrectAccountInitialHash {
                tx_initial_account_hash: expected,
                current_account_hash: current,
            });
        }

        // Ensure nullifiers aren't already present.
        let double_spend = tx
            .get_nullifiers()
            .filter(|nullifier| self.nullifiers.contains(nullifier))
            .collect::<Vec<_>>();
        if !double_spend.is_empty() {
            return Err(VerifyTxError::InputNotesAlreadyConsumed(double_spend));
        }

        // Ensure output notes aren't already present.
        let duplicates = tx
            .output_notes()
            .iter()
            .map(OutputNote::id)
            .filter(|note| self.output_notes.contains_key(note))
            .collect::<Vec<_>>();
        if !duplicates.is_empty() {
            return Err(VerifyTxError::OutputNotesAlreadyExist(duplicates));
        }

        // Ensure that all unauthenticated notes have an inflight output note to consume.
        //
        // We don't need to worry about double spending them since we already checked for
        // that using the nullifiers.
        let missing = tx
            .get_unauthenticated_notes()
            .map(|note| note.id())
            .filter(|note_id| !self.output_notes.contains_key(note_id))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(VerifyTxError::UnauthenticatedNotesNotFound(missing));
        }

        Ok(())
    }

    /// Aggregate the transaction into the state, returning its parent transactions.
    fn insert_transaction(&mut self, tx: &ProvenTransaction) -> BTreeSet<TransactionId> {
        let account_parent = self
            .accounts
            .entry(tx.account_id())
            .or_default()
            .insert(tx.account_update().final_state_hash(), tx.id());

        self.nullifiers.extend(tx.get_nullifiers());
        self.output_notes.extend(
            tx.output_notes().iter().map(|note| (note.id(), OutputNoteState::new(tx.id()))),
        );

        // Authenticated input notes (provably) consume notes that are already committed
        // on chain. They therefore cannot form part of the inflight dependency chain.
        //
        // Additionally, we only care about parents which have not been committed yet.
        let note_parents = tx
            .get_unauthenticated_notes()
            .filter_map(|note| self.output_notes.get(&note.id()))
            .filter_map(|note| note.transaction())
            .copied();

        account_parent.into_iter().chain(note_parents).collect()
    }

    /// Reverts the given state diff.
    ///
    /// # Panics
    ///
    /// Panics if any part of the diff isn't present in the state. Callers should take
    /// care to only revert transaction sets who's ancestors are all either committed or reverted.
    pub fn revert_transactions(&mut self, txs: &[ProvenTransaction]) {
        let diff = StateDelta::new(txs);
        for (account, count) in diff.account_transactions {
            let status = self.accounts.get_mut(&account).expect("Account must exist").revert(count);

            // Prune empty accounts.
            if status.is_empty() {
                self.accounts.remove(&account);
            }
        }

        for nullifier in diff.nullifiers {
            assert!(self.nullifiers.remove(&nullifier), "Nullifier must exist");
        }

        for note in diff.output_notes {
            assert!(self.output_notes.remove(&note).is_some(), "Output note must exist");
        }
    }

    /// Marks the given state diff as committed.
    ///
    /// These transactions are no longer considered inflight. Callers should take care to only
    /// commit transactions who's ancestors are all committed.
    ///
    /// # Panics
    ///
    /// Panics if the accounts don't have enough inflight transactions to commit or if
    /// the output notes don't exist.
    pub fn commit_transactions(&mut self, diff: &StateDelta) {
        for (account, count) in &diff.account_transactions {
            self.accounts.get_mut(account).expect("Account must exist").commit(*count);
        }

        for note in &diff.output_notes {
            self.output_notes.get_mut(note).expect("Output note must exist").commit();
        }
    }

    /// Drops the given state diff from memory.
    ///
    /// # Panics
    ///
    /// Panics if the accounts don't have enough inflight transactions to commit.
    pub fn prune_committed_state(&mut self, diff: StateDelta) {
        for (account, count) in diff.account_transactions {
            let status = self
                .accounts
                .get_mut(&account)
                .expect("Account must exist")
                .prune_commited(count);

            // Prune empty accounts.
            if status.is_empty() {
                self.accounts.remove(&account);
            }
        }

        for nullifier in diff.nullifiers {
            self.nullifiers.remove(&nullifier);
        }

        for output_note in diff.output_notes {
            self.output_notes.remove(&output_note);
        }
    }
}

/// Describes the state of an inflight output note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputNoteState {
    /// Output note is part of a committed block, and its
    /// source transaction should no longer be considered
    /// for dependency tracking.
    Committed,
    /// Output note is still inflight and should be considered
    /// for dependency tracking.
    Inflight(TransactionId),
}

impl OutputNoteState {
    /// Creates a new inflight output note state.
    fn new(tx: TransactionId) -> Self {
        Self::Inflight(tx)
    }

    /// Commits the output note, removing the source transaction.
    fn commit(&mut self) {
        *self = Self::Committed;
    }

    /// Returns the source transaction ID if the output note is not yet committed.
    fn transaction(&self) -> Option<&TransactionId> {
        if let Self::Inflight(tx) = self {
            Some(tx)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use miden_air::Felt;
    use miden_objects::{accounts::AccountType, testing::account_id::AccountIdBuilder};

    use crate::test_utils::{
        mock_account_id, mock_proven_tx,
        note::{mock_note, mock_output_note},
        MockPrivateAccount, MockProvenTxBuilder,
    };

    use super::*;

    #[test]
    fn rejects_duplicate_nullifiers() {
        let account = mock_account_id(1);
        let states = (1u8..=4).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();

        let note_seed = 123;
        // We need to make the note available first, in order for it to be consumed at all.
        let tx0 = MockProvenTxBuilder::with_account(account, states[0].clone(), states[1].clone())
            .output_notes(vec![mock_output_note(note_seed)])
            .build();
        let tx1 = MockProvenTxBuilder::with_account(account, states[1].clone(), states[2].clone())
            .unauthenticated_notes(vec![mock_note(note_seed)])
            .build();
        let tx2 = MockProvenTxBuilder::with_account(account, states[2].clone(), states[3].clone())
            .unauthenticated_notes(vec![mock_note(note_seed)])
            .build();

        let mut uut = InflightState::default();
        uut.add_transaction(&tx0, tx0.account_update().init_state_hash().into())
            .unwrap();
        uut.add_transaction(&tx1, tx1.account_update().init_state_hash().into())
            .unwrap();

        let err = uut.add_transaction(&tx2, None).unwrap_err();

        assert_eq!(
            err,
            VerifyTxError::InputNotesAlreadyConsumed(vec![mock_note(note_seed).nullifier()])
        );
    }

    #[test]
    fn rejects_duplicate_output_notes() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();

        let note = mock_output_note(123);
        let tx0 = MockProvenTxBuilder::with_account(account, states[0].clone(), states[1].clone())
            .output_notes(vec![note.clone()])
            .build();
        let tx1 = MockProvenTxBuilder::with_account(account, states[1].clone(), states[2].clone())
            .output_notes(vec![note.clone()])
            .build();

        let mut uut = InflightState::default();
        uut.add_transaction(&tx0, tx0.account_update().init_state_hash().into())
            .unwrap();

        let err = uut.add_transaction(&tx1, None).unwrap_err();

        assert_eq!(err, VerifyTxError::OutputNotesAlreadyExist(vec![note.id()]));
    }

    #[test]
    fn rejects_account_state_mismatch() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();

        let tx = MockProvenTxBuilder::with_account(account, states[0].clone(), states[1].clone())
            .build();

        let mut uut = InflightState::default();
        let err = uut.add_transaction(&tx, states[2].clone().into()).unwrap_err();

        assert_eq!(
            err,
            VerifyTxError::IncorrectAccountInitialHash {
                tx_initial_account_hash: states[0].clone(),
                current_account_hash: states[2].clone().into()
            }
        );
    }

    #[test]
    fn account_state_transitions() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();

        let tx0 = MockProvenTxBuilder::with_account(account, states[0].clone(), states[1].clone())
            .build();
        let tx1 = MockProvenTxBuilder::with_account(account, states[1].clone(), states[2].clone())
            .build();

        let mut uut = InflightState::default();
        uut.add_transaction(&tx0, states[0].into()).unwrap();
        uut.add_transaction(&tx1, None).unwrap();
    }

    #[test]
    fn new_account_state_defaults_to_zero() {
        let account = mock_account_id(1);

        let tx = MockProvenTxBuilder::with_account(
            account,
            [0u8, 0, 0, 0].into(),
            [1u8, 0, 0, 0].into(),
        )
        .build();

        let mut uut = InflightState::default();
        uut.add_transaction(&tx, None).unwrap();
    }

    #[test]
    fn inflight_account_state_overrides_input_state() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();

        let tx0 = MockProvenTxBuilder::with_account(account, states[0].clone(), states[1].clone())
            .build();
        let tx1 = MockProvenTxBuilder::with_account(account, states[1].clone(), states[2].clone())
            .build();

        let mut uut = InflightState::default();
        uut.add_transaction(&tx0, tx0.account_update().init_state_hash().into())
            .unwrap();

        // Feed in an old state via input. This should be ignored, and the previous tx's final
        // state should be used.
        uut.add_transaction(&tx1, states[0].clone().into()).unwrap();
    }

    #[test]
    fn dependency_tracking() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();
        let note_seed = 123;

        // Parent via account state.
        let tx0 = MockProvenTxBuilder::with_account(account, states[0].clone(), states[1].clone())
            .build();
        // Parent via output note.
        let tx1 = MockProvenTxBuilder::with_account(
            mock_account_id(2),
            states[0].clone(),
            states[1].clone(),
        )
        .output_notes(vec![mock_output_note(note_seed)])
        .build();

        let tx = MockProvenTxBuilder::with_account(account, states[1].clone(), states[2].clone())
            .unauthenticated_notes(vec![mock_note(note_seed)])
            .build();

        let mut uut = InflightState::default();
        uut.add_transaction(&tx0, tx0.account_update().init_state_hash().into())
            .unwrap();
        uut.add_transaction(&tx1, tx1.account_update().init_state_hash().into())
            .unwrap();

        let parents = uut.add_transaction(&tx, None).unwrap();
        let expected = BTreeSet::from([tx0.id(), tx1.id()]);

        assert_eq!(parents, expected);
    }

    #[test]
    fn committed_parents_are_not_tracked() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();
        let note_seed = 123;

        // Parent via account state.
        let tx0 = MockProvenTxBuilder::with_account(account, states[0].clone(), states[1].clone())
            .build();
        // Parent via output note.
        let tx1 = MockProvenTxBuilder::with_account(
            mock_account_id(2),
            states[0].clone(),
            states[1].clone(),
        )
        .output_notes(vec![mock_output_note(note_seed)])
        .build();

        let tx = MockProvenTxBuilder::with_account(account, states[1].clone(), states[2].clone())
            .unauthenticated_notes(vec![mock_note(note_seed)])
            .build();

        let mut uut = InflightState::default();
        uut.add_transaction(&tx0, tx0.account_update().init_state_hash().into())
            .unwrap();
        uut.add_transaction(&tx1, tx1.account_update().init_state_hash().into())
            .unwrap();

        // Commit the parents, which should remove them from dependency tracking.
        let delta = StateDelta::new(&[tx0, tx1]);
        uut.commit_transactions(&delta);

        let parents = uut.add_transaction(&tx, None).unwrap();

        assert!(parents.is_empty());
    }

    #[test]
    fn tx_insertions_and_reversions_cancel_out() {
        // Reverting txs should be equivalent to them never being inserted.
        //
        // We test this by reverting some txs and equating it to the remaining set.
        // This is a form of proprty test.
        let states = (1u8..=5).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();
        let txs = vec![
            MockProvenTxBuilder::with_account(mock_account_id(1), states[0], states[1]),
            MockProvenTxBuilder::with_account(mock_account_id(1), states[1], states[2])
                .output_notes(vec![mock_output_note(111), mock_output_note(222)]),
            MockProvenTxBuilder::with_account(mock_account_id(2), states[0], states[1])
                .unauthenticated_notes(vec![mock_note(222)]),
            MockProvenTxBuilder::with_account(mock_account_id(1), states[2], states[3]),
            MockProvenTxBuilder::with_account(mock_account_id(2), states[1], states[2])
                .unauthenticated_notes(vec![mock_note(111)])
                .output_notes(vec![mock_output_note(45)]),
        ];

        let txs = txs.into_iter().map(MockProvenTxBuilder::build).collect::<Vec<_>>();

        for i in 0..states.len() {
            // Insert all txs and then revert the last `i` of them.
            // This should match only inserting the first `N-i` of them.
            let mut reverted = InflightState::default();
            for (idx, tx) in txs.iter().enumerate() {
                reverted
                    .add_transaction(tx, tx.account_update().init_state_hash().into())
                    .expect(&format!("Inserting tx #{idx} in iteration {i} should succeed"));
            }
            reverted.revert_transactions(&txs[txs.len() - i..]);

            let mut inserted = InflightState::default();
            for (idx, tx) in txs.iter().rev().skip(i).rev().enumerate() {
                inserted
                    .add_transaction(tx, tx.account_update().init_state_hash().into())
                    .expect(&format!("Inserting tx #{idx} in iteration {i} should succeed"));
            }

            assert_eq!(reverted, inserted, "Iteration {i}");
        }
    }

    #[test]
    fn pruning_committed_state() {
        //! This is a form of property test, where we assert that pruning the first `i` of `N`
        //! transactions is equivalent to only inserting the last `N-i` transactions.
        let states = (1u8..=5).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();

        // Skipping initial txs means that output notes required for subsequent unauthenticated
        // input notes wont' always be present. To work around this, we instead only use authenticated
        // input notes.
        let txs = vec![
            MockProvenTxBuilder::with_account(mock_account_id(1), states[0], states[1]),
            MockProvenTxBuilder::with_account(mock_account_id(1), states[1], states[2])
                .output_notes(vec![mock_output_note(111), mock_output_note(222)]),
            MockProvenTxBuilder::with_account(mock_account_id(2), states[0], states[1])
                .nullifiers(vec![mock_note(222).nullifier()]),
            MockProvenTxBuilder::with_account(mock_account_id(1), states[2], states[3]),
            MockProvenTxBuilder::with_account(mock_account_id(2), states[1], states[2])
                .nullifiers(vec![mock_note(111).nullifier()])
                .output_notes(vec![mock_output_note(45)]),
        ];

        let txs = txs.into_iter().map(MockProvenTxBuilder::build).collect::<Vec<_>>();

        for i in 0..states.len() {
            // Insert all txs and then commit and prune the first `i` of them.
            //
            // This should match only inserting the final `N-i` transactions.
            let mut committed = InflightState::default();
            for (idx, tx) in txs.iter().enumerate() {
                committed
                    .add_transaction(tx, tx.account_update().init_state_hash().into())
                    .expect(&format!("Inserting tx #{idx} in iteration {i} should succeed"));
            }
            let delta = StateDelta::new(&txs[..i]);
            committed.commit_transactions(&delta);
            committed.prune_committed_state(delta);

            let mut inserted = InflightState::default();
            for (idx, tx) in txs.iter().skip(i).enumerate() {
                inserted
                    .add_transaction(&tx, tx.account_update().init_state_hash().into())
                    .expect(&format!("Inserting tx #{idx} in iteration {i} should succeed"));
            }

            assert_eq!(committed, inserted, "Iteration {i}");
        }
    }
}
