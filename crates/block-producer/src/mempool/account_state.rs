use std::collections::{BTreeMap, BTreeSet, VecDeque};

use miden_objects::{accounts::AccountId, transaction::TransactionId, Digest};

/// Tracks the committed and inflight account states.
///
/// Allows appending and reverting transactions as well as marking them
/// as part of a committed block. Committed state can also be removed once the
/// state is considered past the stale threshold.
///
/// Accounts which are considered empty (no inflight or committed state) are actively
/// pruned.
#[derive(Default)]
pub struct AccountStates {
    /// Non-empty inflight account states.
    ///
    /// Accounts which are [AccountStatus::Empty] are immedietely pruned.
    accounts: BTreeMap<AccountId, AccountState>,
}

impl AccountStates {
    /// The current inflight account state, if any.
    pub fn get(&self, account: &AccountId) -> Option<&Digest> {
        self.accounts
            .get(account)
            .map(|account_state| account_state.latest_state())
            .flatten()
    }

    /// Inserts a new transaction and its state, returning the previous inflight transaction, if any.
    pub fn insert(
        &mut self,
        account: AccountId,
        state: Digest,
        transaction: TransactionId,
    ) -> Option<TransactionId> {
        self.accounts.entry(account).or_default().insert(state, transaction)
    }

    /// Reverts the most recent `n` inflight transactions of the given account.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` inflight transactions for the account or if the account has no committed or inflight state.
    pub fn revert_transactions(&mut self, account: &AccountId, n: usize) {
        let status = self.accounts.get_mut(account).expect("Account must exist").revert(n);

        // Prune empty accounts.
        if status.is_empty() {
            self.accounts.remove(account);
        }
    }

    /// Mark the oldest `n` inflight transactions as committed i.e. in a committed block.
    ///
    /// These transactions are no longer considered inflight.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` inflight transactions for the account or if the account has no committed or inflight state.
    pub fn commit_transactions(&mut self, account: &AccountId, n: usize) {
        self.accounts.get_mut(account).expect("Account must exist").commit(n);
    }

    /// Remove the oldest `n` committed transactions.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` committed transactions in the account.
    pub fn remove_committed_state(&mut self, account: &AccountId, n: usize) {
        let status = self.accounts.get_mut(account).expect("Account must exist").remove_commited(n);

        // Prune empty accounts.
        if status.is_empty() {
            self.accounts.remove(account);
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
