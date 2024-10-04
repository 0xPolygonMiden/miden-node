use std::collections::{BTreeMap, BTreeSet, VecDeque};

use miden_objects::{
    accounts::AccountId,
    notes::Nullifier,
    transaction::{ProvenTransaction, TransactionId},
    Digest,
};

use crate::{errors::AddTransactionErrorRework, store::TransactionInputs};

use super::StateDiff;

/// Tracks the inflight state of the mempool. This includes recently committed blocks.
///
/// Allows appending and reverting transactions as well as marking them
/// as part of a committed block. Committed state can also be removed once the
/// state is considered past the stale threshold.
#[derive(Default)]
pub struct InflightState {
    /// Non-empty inflight account states.
    ///
    /// Accounts which are [AccountStatus::Empty] are immedietely pruned.
    accounts: BTreeMap<AccountId, AccountState>,

    /// Nullifiers emitted by inflight transactions and recently committed blocks.
    nullifiers: BTreeSet<Nullifier>,
}

impl InflightState {
    /// Appends the transaction to the inflight state.
    ///
    /// This operation is atomic i.e. a rejected transaction has no impact of the state.
    pub fn add_transaction(
        &mut self,
        tx: &ProvenTransaction,
        inputs: &TransactionInputs,
    ) -> Result<BTreeSet<TransactionId>, AddTransactionErrorRework> {
        // A rejected transaction must not affect the state, so we separate verification and mutation of the state.
        self.verify_transaction(tx, inputs)?;

        Ok(self.insert_transaction(tx))
    }

    fn verify_transaction(
        &mut self,
        tx: &ProvenTransaction,
        inputs: &TransactionInputs,
    ) -> Result<(), AddTransactionErrorRework> {
        // Ensure current account state is correct.
        let current = self
            .accounts
            .get(&tx.account_id())
            .map(|account_state| account_state.latest_state())
            .flatten()
            .copied()
            .or(inputs.account_hash)
            .unwrap_or_default();
        let expected = tx.account_update().init_state_hash();

        if expected != current {
            return Err(AddTransactionErrorRework::InvalidAccountState { current, expected });
        }

        // Ensure nullifiers aren't already present.
        // TODO: Verifying the inputs nullifiers should be done externally already.
        // TODO: The above should cause a change in type for inputs indicating this.
        let input_nullifiers = tx.get_nullifiers().collect::<BTreeSet<_>>();
        let double_spend =
            self.nullifiers.union(&input_nullifiers).copied().collect::<BTreeSet<_>>();
        if !double_spend.is_empty() {
            return Err(AddTransactionErrorRework::NotesAlreadyConsumed(double_spend));
        }

        // TODO: additional input and output note checks, depends on the transaction type changes.

        Ok(())
    }

    /// Aggregate the transaction into the state, returning the parent transactions.
    fn insert_transaction(&mut self, tx: &ProvenTransaction) -> BTreeSet<TransactionId> {
        let account_parent = self
            .accounts
            .entry(tx.account_id())
            .or_default()
            .insert(tx.account_update().final_state_hash(), tx.id());

        self.nullifiers.extend(tx.get_nullifiers());

        // TODO: input and output notes

        account_parent.into_iter().collect()
    }

    /// Reverts the given state diff.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` inflight transactions for the account or if the account has no committed or inflight state.
    pub fn revert_transactions(&mut self, diff: StateDiff) {
        for (account, count) in diff.account_transactions {
            let status = self.accounts.get_mut(&account).expect("Account must exist").revert(count);

            // Prune empty accounts.
            if status.is_empty() {
                self.accounts.remove(&account);
            }
        }

        for nullifier in diff.nullifiers {
            self.nullifiers.remove(&nullifier);
        }

        // TODO: input and output notes
    }

    /// Mark the oldest `n` inflight transactions as committed i.e. in a committed block.
    ///
    /// These transactions are no longer considered inflight.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` inflight transactions for the account or if the account has no committed or inflight state.
    pub fn commit_transactions(&mut self, diff: &StateDiff) {
        for (account, count) in &diff.account_transactions {
            self.accounts.get_mut(account).expect("Account must exist").commit(*count);
        }
    }

    /// Drops the given state diff from memory.
    pub fn remove_committed_state(&mut self, diff: StateDiff) {
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
    /// Inserts a new transaction and its state, returning the previous inflight transaction, if any.
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
/// Is marked as #[must_use] so that callers must prune empty accounts.
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
