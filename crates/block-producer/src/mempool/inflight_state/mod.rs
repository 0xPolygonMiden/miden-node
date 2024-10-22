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
    transaction::AuthenticatedTransaction,
};

mod account_state;

use account_state::InflightAccountState;

use super::BlockNumber;

// IN-FLIGHT STATE
// ================================================================================================

/// Tracks the inflight state of the mempool. This includes recently committed blocks.
///
/// Allows appending and reverting transactions as well as marking them as part of a committed
/// block. Committed state can also be pruned once the state is considered past the stale
/// threshold.
#[derive(Clone, Debug, PartialEq)]
pub struct InflightState {
    /// Account states from inflight transactions.
    ///
    /// Accounts which are [AccountStatus::Empty] are immediately pruned.
    accounts: BTreeMap<AccountId, InflightAccountState>,

    /// Nullifiers produced by the input notes of inflight transactions.
    nullifiers: BTreeSet<Nullifier>,

    /// Notes created by inflight transactions.
    ///
    /// Some of these may already be consumed - check the nullifiers.
    output_notes: BTreeMap<NoteId, OutputNoteState>,

    /// Delta's representing the impact of each recently committed blocks on the inflight state.
    ///
    /// These are used to prune committed state after `num_retained_blocks` have passed.
    committed_state: VecDeque<StateDelta>,

    /// Amount of recently committed blocks we retain in addition to the inflight state.
    ///
    /// This provides an overlap between committed and inflight state, giving a grace
    /// period for incoming transactions to be verified against both without requiring it
    /// to be an atomic action.
    num_retained_blocks: usize,

    /// The latest committed block height.
    chain_tip: BlockNumber,
}

/// The aggregated impact of a set of sequential transactions on the [InflightState].
#[derive(Clone, Default, Debug, PartialEq)]
struct StateDelta {
    /// The number of transactions that affected each account.
    account_transactions: BTreeMap<AccountId, usize>,

    /// The nullifiers consumed by the transactions.
    nullifiers: BTreeSet<Nullifier>,

    /// The notes produced by the transactions.
    output_notes: BTreeSet<NoteId>,
}

impl StateDelta {
    fn new(txs: &[AuthenticatedTransaction]) -> Self {
        let mut account_transactions = BTreeMap::<AccountId, usize>::new();
        let mut nullifiers = BTreeSet::new();
        let mut output_notes = BTreeSet::new();

        for tx in txs {
            *account_transactions.entry(tx.account_id()).or_default() += 1;
            nullifiers.extend(tx.nullifiers());
            output_notes.extend(tx.output_notes());
        }

        Self {
            account_transactions,
            nullifiers,
            output_notes,
        }
    }
}

impl InflightState {
    /// Creates an [InflightState] which will retain committed state for the given
    /// amount of blocks before pruning them.
    pub fn new(chain_tip: BlockNumber, num_retained_blocks: usize) -> Self {
        Self {
            num_retained_blocks,
            chain_tip,
            accounts: Default::default(),
            nullifiers: Default::default(),
            output_notes: Default::default(),
            committed_state: Default::default(),
        }
    }

    /// Appends the transaction to the inflight state.
    ///
    /// This operation is atomic i.e. a rejected transaction has no impact of the state.
    pub fn add_transaction(
        &mut self,
        tx: &AuthenticatedTransaction,
    ) -> Result<BTreeSet<TransactionId>, AddTransactionError> {
        // Separate verification and state mutation so that a rejected transaction
        // does not impact the state (atomicity).
        self.verify_transaction(tx)?;

        let parents = self.insert_transaction(tx);

        Ok(parents)
    }

    fn oldest_committed_state(&self) -> BlockNumber {
        let committed_len: u32 = self
            .committed_state
            .len()
            .try_into()
            .expect("We should not be storing many blocks");
        self.chain_tip
            .checked_sub(BlockNumber::new(committed_len))
            .expect("Chain height cannot be less than number of committed blocks")
    }

    fn verify_transaction(&self, tx: &AuthenticatedTransaction) -> Result<(), AddTransactionError> {
        // The mempool retains recently committed blocks, in addition to the state that is currently
        // inflight. This overlap with the committed state allows us to verify incoming
        // transactions against the current state (committed + inflight). Transactions are
        // first authenticated against the committed state prior to being submitted to the
        // mempool. The overlap provides a grace period between transaction authentication
        // against committed state and verification against inflight state.
        //
        // Here we just ensure that this authentication point is still within this overlap zone.
        // This should only fail if the grace period is too restrictive for the current
        // combination of block rate, transaction throughput and database IO.
        let stale_limit = self.oldest_committed_state();
        if tx.authentication_height() < stale_limit {
            return Err(AddTransactionError::StaleInputs {
                input_block: tx.authentication_height(),
                stale_limit,
            });
        }

        // Ensure current account state is correct.
        let current = self
            .accounts
            .get(&tx.account_id())
            .and_then(|account_state| account_state.current_state())
            .copied()
            .or(tx.store_account_state());
        let expected = tx.account_update().init_state_hash();

        if expected != current.unwrap_or_default() {
            return Err(VerifyTxError::IncorrectAccountInitialHash {
                tx_initial_account_hash: expected,
                current_account_hash: current,
            }
            .into());
        }

        // Ensure nullifiers aren't already present.
        let double_spend = tx
            .nullifiers()
            .filter(|nullifier| self.nullifiers.contains(nullifier))
            .collect::<Vec<_>>();
        if !double_spend.is_empty() {
            return Err(VerifyTxError::InputNotesAlreadyConsumed(double_spend).into());
        }

        // Ensure output notes aren't already present.
        let duplicates = tx
            .output_notes()
            .filter(|note| self.output_notes.contains_key(note))
            .collect::<Vec<_>>();
        if !duplicates.is_empty() {
            return Err(VerifyTxError::OutputNotesAlreadyExist(duplicates).into());
        }

        // Ensure that all unauthenticated notes have an inflight output note to consume.
        //
        // We don't need to worry about double spending them since we already checked for
        // that using the nullifiers.
        let missing = tx
            .unauthenticated_notes()
            .filter(|note_id| !self.output_notes.contains_key(note_id))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(VerifyTxError::UnauthenticatedNotesNotFound(missing).into());
        }

        Ok(())
    }

    /// Aggregate the transaction into the state, returning its parent transactions.
    fn insert_transaction(&mut self, tx: &AuthenticatedTransaction) -> BTreeSet<TransactionId> {
        let account_parent = self
            .accounts
            .entry(tx.account_id())
            .or_default()
            .insert(tx.account_update().final_state_hash(), tx.id());

        self.nullifiers.extend(tx.nullifiers());
        self.output_notes
            .extend(tx.output_notes().map(|note_id| (note_id, OutputNoteState::new(tx.id()))));

        // Authenticated input notes (provably) consume notes that are already committed
        // on chain. They therefore cannot form part of the inflight dependency chain.
        //
        // Additionally, we only care about parents which have not been committed yet.
        let note_parents = tx
            .unauthenticated_notes()
            .filter_map(|note_id| self.output_notes.get(&note_id))
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
    pub fn revert_transactions(&mut self, txs: &[AuthenticatedTransaction]) {
        let delta = StateDelta::new(txs);
        for (account, count) in delta.account_transactions {
            let status = self.accounts.get_mut(&account).expect("Account must exist").revert(count);

            // Prune empty accounts.
            if status.is_empty() {
                self.accounts.remove(&account);
            }
        }

        for nullifier in delta.nullifiers {
            assert!(self.nullifiers.remove(&nullifier), "Nullifier must exist");
        }

        for note in delta.output_notes {
            assert!(self.output_notes.remove(&note).is_some(), "Output note must exist");
        }
    }

    /// Marks the given state diff as committed.
    ///
    /// These transactions are no longer considered inflight. Callers should take care to only
    /// commit transactions who's ancestors are all committed.
    ///
    /// Note that this state is still retained for the configured number of blocks. The oldest
    /// retained block is also pruned from the state.
    ///
    /// # Panics
    ///
    /// Panics if the accounts don't have enough inflight transactions to commit or if
    /// the output notes don't exist.
    pub fn commit_block(&mut self, txs: &[AuthenticatedTransaction]) {
        let delta = StateDelta::new(txs);
        for (account, count) in &delta.account_transactions {
            self.accounts.get_mut(account).expect("Account must exist").commit(*count);
        }

        for note in &delta.output_notes {
            self.output_notes.get_mut(note).expect("Output note must exist").commit();
        }

        self.committed_state.push_back(delta);

        if self.committed_state.len() > self.num_retained_blocks {
            let delta = self.committed_state.pop_front().expect("Must be some due to length check");
            self.prune_committed_state(delta);
        }

        self.chain_tip.increment();
    }

    /// Removes the delta from inflight state.
    ///
    /// # Panics
    ///
    /// Panics if the accounts don't have enough inflight transactions to commit.
    fn prune_committed_state(&mut self, diff: StateDelta) {
        for (account, count) in diff.account_transactions {
            let status = self
                .accounts
                .get_mut(&account)
                .expect("Account must exist")
                .prune_committed(count);

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
    /// Output note is part of a committed block, and its source transaction should no longer be
    /// considered for dependency tracking.
    Committed,
    /// Output note is still inflight and should be considered for dependency tracking.
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

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use miden_air::Felt;
    use miden_objects::{accounts::AccountType, testing::account_id::AccountIdBuilder};

    use super::*;
    use crate::test_utils::{
        mock_account_id, mock_proven_tx,
        note::{mock_note, mock_output_note},
        MockPrivateAccount, MockProvenTxBuilder,
    };

    #[test]
    fn rejects_duplicate_nullifiers() {
        let account = mock_account_id(1);
        let states = (1u8..=4).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();

        let note_seed = 123;
        // We need to make the note available first, in order for it to be consumed at all.
        let tx0 = MockProvenTxBuilder::with_account(account, states[0], states[1])
            .output_notes(vec![mock_output_note(note_seed)])
            .build();
        let tx1 = MockProvenTxBuilder::with_account(account, states[1], states[2])
            .unauthenticated_notes(vec![mock_note(note_seed)])
            .build();
        let tx2 = MockProvenTxBuilder::with_account(account, states[2], states[3])
            .unauthenticated_notes(vec![mock_note(note_seed)])
            .build();

        let mut uut = InflightState::new(BlockNumber::default(), 1);
        uut.add_transaction(&AuthenticatedTransaction::from_inner(tx0)).unwrap();
        uut.add_transaction(&AuthenticatedTransaction::from_inner(tx1)).unwrap();

        let err = uut.add_transaction(&AuthenticatedTransaction::from_inner(tx2)).unwrap_err();

        assert_eq!(
            err,
            VerifyTxError::InputNotesAlreadyConsumed(vec![mock_note(note_seed).nullifier()]).into()
        );
    }

    #[test]
    fn rejects_duplicate_output_notes() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();

        let note = mock_output_note(123);
        let tx0 = MockProvenTxBuilder::with_account(account, states[0], states[1])
            .output_notes(vec![note.clone()])
            .build();
        let tx1 = MockProvenTxBuilder::with_account(account, states[1], states[2])
            .output_notes(vec![note.clone()])
            .build();

        let mut uut = InflightState::new(BlockNumber::default(), 1);
        uut.add_transaction(&AuthenticatedTransaction::from_inner(tx0)).unwrap();

        let err = uut.add_transaction(&AuthenticatedTransaction::from_inner(tx1)).unwrap_err();

        assert_eq!(err, VerifyTxError::OutputNotesAlreadyExist(vec![note.id()]).into());
    }

    #[test]
    fn rejects_account_state_mismatch() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();

        let tx = MockProvenTxBuilder::with_account(account, states[0], states[1]).build();

        let mut uut = InflightState::new(BlockNumber::default(), 1);
        let err = uut
            .add_transaction(&AuthenticatedTransaction::from_inner(tx).with_store_state(states[2]))
            .unwrap_err();

        assert_eq!(
            err,
            VerifyTxError::IncorrectAccountInitialHash {
                tx_initial_account_hash: states[0],
                current_account_hash: states[2].into()
            }
            .into()
        );
    }

    #[test]
    fn account_state_transitions() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();

        let tx0 = MockProvenTxBuilder::with_account(account, states[0], states[1]).build();
        let tx1 = MockProvenTxBuilder::with_account(account, states[1], states[2]).build();

        let mut uut = InflightState::new(BlockNumber::default(), 1);
        uut.add_transaction(&AuthenticatedTransaction::from_inner(tx0)).unwrap();
        uut.add_transaction(&AuthenticatedTransaction::from_inner(tx1).with_empty_store_state())
            .unwrap();
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

        let mut uut = InflightState::new(BlockNumber::default(), 1);
        uut.add_transaction(&AuthenticatedTransaction::from_inner(tx).with_empty_store_state())
            .unwrap();
    }

    #[test]
    fn inflight_account_state_overrides_input_state() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();

        let tx0 = MockProvenTxBuilder::with_account(account, states[0], states[1]).build();
        let tx1 = MockProvenTxBuilder::with_account(account, states[1], states[2]).build();

        let mut uut = InflightState::new(BlockNumber::default(), 1);
        uut.add_transaction(&AuthenticatedTransaction::from_inner(tx0)).unwrap();

        // Feed in an old state via input. This should be ignored, and the previous tx's final
        // state should be used.
        uut.add_transaction(&AuthenticatedTransaction::from_inner(tx1).with_store_state(states[0]))
            .unwrap();
    }

    #[test]
    fn dependency_tracking() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();
        let note_seed = 123;

        // Parent via account state.
        let tx0 = MockProvenTxBuilder::with_account(account, states[0], states[1]).build();
        // Parent via output note.
        let tx1 = MockProvenTxBuilder::with_account(mock_account_id(2), states[0], states[1])
            .output_notes(vec![mock_output_note(note_seed)])
            .build();

        let tx = MockProvenTxBuilder::with_account(account, states[1], states[2])
            .unauthenticated_notes(vec![mock_note(note_seed)])
            .build();

        let mut uut = InflightState::new(BlockNumber::default(), 1);
        uut.add_transaction(&AuthenticatedTransaction::from_inner(tx0.clone())).unwrap();
        uut.add_transaction(&AuthenticatedTransaction::from_inner(tx1.clone())).unwrap();

        let parents = uut
            .add_transaction(&AuthenticatedTransaction::from_inner(tx).with_empty_store_state())
            .unwrap();
        let expected = BTreeSet::from([tx0.id(), tx1.id()]);

        assert_eq!(parents, expected);
    }

    #[test]
    fn committed_parents_are_not_tracked() {
        let account = mock_account_id(1);
        let states = (1u8..=3).map(|x| Digest::from([x, 0, 0, 0])).collect::<Vec<_>>();
        let note_seed = 123;

        // Parent via account state.
        let tx0 = MockProvenTxBuilder::with_account(account, states[0], states[1]).build();
        let tx0 = AuthenticatedTransaction::from_inner(tx0);
        // Parent via output note.
        let tx1 = MockProvenTxBuilder::with_account(mock_account_id(2), states[0], states[1])
            .output_notes(vec![mock_output_note(note_seed)])
            .build();
        let tx1 = AuthenticatedTransaction::from_inner(tx1);

        let tx = MockProvenTxBuilder::with_account(account, states[1], states[2])
            .unauthenticated_notes(vec![mock_note(note_seed)])
            .build();

        let mut uut = InflightState::new(BlockNumber::default(), 1);
        uut.add_transaction(&tx0.clone()).unwrap();
        uut.add_transaction(&tx1.clone()).unwrap();

        // Commit the parents, which should remove them from dependency tracking.
        uut.commit_block(&[tx0, tx1]);

        let parents = uut
            .add_transaction(&AuthenticatedTransaction::from_inner(tx).with_empty_store_state())
            .unwrap();

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

        let txs = txs
            .into_iter()
            .map(MockProvenTxBuilder::build)
            .map(AuthenticatedTransaction::from_inner)
            .collect::<Vec<_>>();

        for i in 0..states.len() {
            // Insert all txs and then revert the last `i` of them.
            // This should match only inserting the first `N-i` of them.
            let mut reverted = InflightState::new(BlockNumber::default(), 1);
            for (idx, tx) in txs.iter().enumerate() {
                reverted.add_transaction(tx).unwrap_or_else(|err| {
                    panic!("Inserting tx #{idx} in iteration {i} should succeed: {err}")
                });
            }
            reverted.revert_transactions(&txs[txs.len() - i..]);

            let mut inserted = InflightState::new(BlockNumber::default(), 1);
            for (idx, tx) in txs.iter().rev().skip(i).rev().enumerate() {
                inserted.add_transaction(tx).unwrap_or_else(|err| {
                    panic!("Inserting tx #{idx} in iteration {i} should succeed: {err}")
                });
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
        // input notes wont' always be present. To work around this, we instead only use
        // authenticated input notes.
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

        let txs = txs
            .into_iter()
            .map(MockProvenTxBuilder::build)
            .map(AuthenticatedTransaction::from_inner)
            .collect::<Vec<_>>();

        for i in 0..states.len() {
            // Insert all txs and then commit and prune the first `i` of them.
            //
            // This should match only inserting the final `N-i` transactions.
            // Note: we force all committed state to immedietely be pruned by setting
            // it to zero.
            let mut committed = InflightState::new(BlockNumber::default(), 0);
            for (idx, tx) in txs.iter().enumerate() {
                committed.add_transaction(tx).unwrap_or_else(|err| {
                    panic!("Inserting tx #{idx} in iteration {i} should succeed: {err}")
                });
            }
            committed.commit_block(&txs[..i]);

            let mut inserted = InflightState::new(BlockNumber::default(), 0);
            for (idx, tx) in txs.iter().skip(i).enumerate() {
                inserted.add_transaction(tx).unwrap_or_else(|err| {
                    panic!("Inserting tx #{idx} in iteration {i} should succeed: {err}")
                });
            }

            assert_eq!(committed, inserted, "Iteration {i}");
        }
    }
}
