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

/// Tracks the inflight state of an account.
#[derive(Default, Debug, PartialEq)]
pub struct InflightAccountState {
    /// This account's state transitions due to inflight transactions.
    ///
    /// Ordering is chronological from front (oldest) to back (newest).
    states: VecDeque<(Digest, TransactionId)>,

    /// The number of states that have been committed.
    ///
    /// This effectively acts as a pivot point for `self.states`, splitting it into two segments.
    /// The first contains committed states and the second those that are uncommitted.
    committed: usize,
}

impl InflightAccountState {
    /// Inserts a new state update, returning the parent transaction ID if its uncommitted.
    pub fn insert(&mut self, state: Digest, tx: TransactionId) -> Option<TransactionId> {
        let mut parent = self.states.back().map(|(_, tx)| tx).copied();

        // The parent is only valid if its uncommitted.
        if self.uncommitted_count() == 0 {
            parent.take();
        }

        self.states.push_back((state, tx));

        parent
    }

    /// The latest state of this account.
    pub fn current_state(&self) -> Option<&Digest> {
        self.states.back().map(|(state, _)| state)
    }

    /// Reverts the most recent `n` uncommitted inflight transactions.
    ///
    /// # Returns
    ///
    /// Returns the emptyness state of the account.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` uncommitted inflight transactions.
    pub fn revert(&mut self, n: usize) -> AccountStatus {
        let uncommitted = self.uncommitted_count();
        assert!(
            uncommitted >= n, "Attempted to revert {n} transactions which is more than the {uncommitted} which are uncommitted.",
        );

        self.states.drain(self.states.len() - n..);

        self.emptyness()
    }

    /// Commits the next `n` uncommitted inflight transactions.
    ///
    /// # Panics
    ///
    /// Panics if there are less than `n` uncommitted inflight transactions.
    pub fn commit(&mut self, n: usize) {
        let uncommitted = self.uncommitted_count();
        assert!(
            uncommitted >= n, "Attempted to revert {n} transactions which is more than the {uncommitted} which are uncommitted."
        );

        self.committed += n;
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
    pub fn prune_commited(&mut self, n: usize) -> AccountStatus {
        assert!(
            self.committed >= n,
            "Attempted to prune {n} transactions, which is more than the {} which are committed",
            self.committed
        );

        self.committed -= n;
        self.states.drain(..n);

        self.emptyness()
    }

    /// This is essentially `is_empty` with the additional benefit that
    /// [AccountStatus] is marked as `#[must_use]`, forcing callers to
    /// handle empty accounts (which should be pruned).
    fn emptyness(&self) -> AccountStatus {
        if self.states.is_empty() {
            AccountStatus::Empty
        } else {
            AccountStatus::NonEmpty
        }
    }

    /// The number of uncommitted inflight transactions.
    fn uncommitted_count(&self) -> usize {
        self.states.len() - self.committed
    }
}

/// Describes the emptyness of an [AccountState].
///
/// Is marked as #[must_use] so that callers handle prune empty accounts.
#[must_use]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccountStatus {
    /// [AccountState] contains no state and should be pruned.
    Empty,
    /// [AccountState] contains state and should be kept.
    NonEmpty,
}

impl AccountStatus {
    pub fn is_empty(&self) -> bool {
        *self == Self::Empty
    }
}
