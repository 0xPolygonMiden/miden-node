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

/// Tracks the inflight state of the mempool. This includes recently committed blocks.
///
/// Allows appending and reverting transactions as well as marking them
/// as part of a committed block. Committed state can also be pruned once the
/// state is considered past the stale threshold.
#[derive(Default)]
pub struct InflightState {
    /// Non-empty inflight account states.
    ///
    /// Accounts which are [AccountStatus::Empty] are immedietely pruned.
    accounts: BTreeMap<AccountId, AccountState>,

    /// Nullifiers consumed by the input notes of inflight transactions.
    nullifiers: BTreeSet<Nullifier>,

    /// Notes created by inflight transactions.
    ///
    /// Some of these may already be consumed - check the nullifiers.
    output_notes: BTreeMap<NoteId, TransactionId>,
}

/// The aggregated impact of a set of sequential transactions on the [InflightState].
pub struct StateDiff {
    /// The number of transactions that affected each account.
    account_transactions: BTreeMap<AccountId, usize>,

    /// The nullifiers consumed by the transactions.
    nullifiers: BTreeSet<Nullifier>,

    /// The notes produced by the transactions.
    output_notes: BTreeSet<NoteId>,
}

impl StateDiff {
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
    ) -> Result<BTreeSet<TransactionId>, AddTransactionError> {
        // Separate verification and state mutation so that a rejected transaction
        // does not impact the state (atomicity).
        self.verify_transaction(tx, input_account_hash)?;

        let parents = self.insert_transaction(tx);

        Ok(parents)
    }

    fn verify_transaction(
        &mut self,
        tx: &ProvenTransaction,
        input_account_hash: Option<Digest>,
    ) -> Result<(), VerifyTxError> {
        // Ensure current account state is correct.
        let current = self
            .accounts
            .get(&tx.account_id())
            .and_then(|account_state| account_state.latest_state())
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
            .filter(|note_id| self.output_notes.contains_key(note_id))
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
        self.output_notes
            .extend(tx.output_notes().iter().map(|note| (note.id(), tx.id())));

        // Authenticated input notes (provably) consume notes that are already committed
        // on chain. They therefore cannot form part of the inflight dependency chain.
        //
        // i.e. we only care about unauthenticated notes here.
        let note_parents = tx
            .get_unauthenticated_notes()
            .filter_map(|note| self.output_notes.get(&note.id()))
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
        let diff = StateDiff::new(txs);
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
    /// Panics if the accounts don't have enough inflight transactions to commit.
    pub fn commit_transactions(&mut self, diff: &StateDiff) {
        for (account, count) in &diff.account_transactions {
            self.accounts.get_mut(account).expect("Account must exist").commit(*count);
        }
    }

    /// Drops the given state diff from memory.
    ///
    /// # Panics
    ///
    /// Panics if the accounts don't have enough inflight transactions to commit.
    pub fn prune_committed_state(&mut self, diff: StateDiff) {
        for (account, count) in diff.account_transactions {
            let status = self
                .accounts
                .get_mut(&account)
                .expect("Account must exist")
                .remove_commited(count);

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

/// Tracks the state of a single account.
#[derive(Default)]
struct AccountState {
    /// State progression of this account over all uncommitted inflight transactions.
    ///
    /// Ordering is chronological from front (oldest) to back (newest).
    inflight: VecDeque<(Digest, TransactionId)>,

    /// The latest committed state.
    ///
    /// Only valid if the committed count is greater than zero.
    committed_state: Digest,

    /// The number of committed transactions.
    ///
    /// If this is zero then the committed state is meaningless.
    committed_count: usize,
}

impl AccountState {
    /// Inserts a new transaction and its state, returning the previous inflight transaction, if
    /// any.
    pub fn insert(&mut self, state: Digest, tx: TransactionId) -> Option<TransactionId> {
        let previous = self.inflight.back().map(|(_, tx)| tx).copied();

        self.inflight.push_back((state, tx));

        previous
    }

    /// The latest state of this account.
    pub fn latest_state(&self) -> Option<&Digest> {
        self.inflight
            .back()
            .map(|(state, _)| state)
            // A count of zero indicates no committed state.
            .or((self.committed_count > 1).then_some(&self.committed_state))
    }

    /// Reverts the most recent `n` inflight transactions.
    ///
    /// # Returns
    ///
    /// Returns the emptyness state of the account.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` inflight transactions.
    pub fn revert(&mut self, n: usize) -> AccountStatus {
        let length = self.inflight.len();
        assert!(
            length >= n, "Attempted to revert {n} transactions which is more than the {length} which are inflight.",
        );

        self.inflight.drain(length - n..);

        self.emptyness()
    }

    /// Commits the first `n` inflight transactions.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` inflight transactions.
    pub fn commit(&mut self, n: usize) {
        if n == 0 {
            return;
        }

        let length = self.inflight.len();
        assert!(
            length >= n, "Attempted to revert {n} transactions which is more than the {length} which are inflight.",
        );

        self.committed_state = self
            .inflight
            .drain(..n)
            .last()
            .map(|(state, _)| state)
            .expect("Must be Some as n > 0");
        self.committed_count += n;
    }

    /// Removes `n` committed transactions.
    ///
    /// # Returns
    ///
    /// Returns the emptyness state of the account.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` committed transactions.
    pub fn remove_commited(&mut self, n: usize) -> AccountStatus {
        assert!(
            n <= self.committed_count,
            "Attempted to remove {n} committed transactions, but only {} are present.",
            self.committed_count
        );
        self.committed_count -= n;

        self.emptyness()
    }

    /// This is essentially `is_empty` with the additional benefit that
    /// [AccountStatus] is marked as `#[must_use]`, forcing callers to
    /// handle empty accounts (which should be pruned).
    fn emptyness(&self) -> AccountStatus {
        if self.inflight.is_empty() && self.committed_count == 0 {
            AccountStatus::Empty
        } else {
            AccountStatus::NonEmpty
        }
    }
}

/// Describes the emptyness of an [AccountState].
///
/// Is marked as #[must_use] so that callers handle prune empty accounts.
#[must_use]
#[derive(Clone, Copy, PartialEq, Eq)]
enum AccountStatus {
    /// [AccountState] contains no state and should be pruned.
    Empty,
    /// [AccountState] contains state and should be kept.
    NonEmpty,
}

impl AccountStatus {
    fn is_empty(&self) -> bool {
        *self == Self::Empty
    }
}
