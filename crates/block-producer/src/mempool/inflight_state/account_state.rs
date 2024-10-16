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

    /// The number of inflight states that have been committed.
    ///
    /// This acts as a pivot index for `self.states`, splitting it into two segments.
    /// The first contains committed states and the second those that are uncommitted.
    committed: usize,
}

impl InflightAccountState {
    /// Appends the new state, returning the previous state's transaction ID __IFF__ it is
    /// uncommitted.
    pub fn insert(&mut self, state: Digest, tx: TransactionId) -> Option<TransactionId> {
        let mut parent = self.states.back().map(|(_, tx)| tx).copied();

        // Only return uncommitted parent ID.
        //
        // Since this is the latest state's ID, we need at least one uncommitted state.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{random_digest, random_tx_id};

    #[test]
    fn current_state_is_the_most_recently_inserted() {
        let mut uut = InflightAccountState::default();
        uut.insert(random_digest(), random_tx_id());
        uut.insert(random_digest(), random_tx_id());
        uut.insert(random_digest(), random_tx_id());

        let expected = random_digest();
        uut.insert(expected, random_tx_id());

        assert_eq!(uut.current_state(), Some(&expected));
    }

    #[test]
    fn parent_is_the_most_recently_inserted() {
        let mut uut = InflightAccountState::default();
        uut.insert(random_digest(), random_tx_id());
        uut.insert(random_digest(), random_tx_id());
        uut.insert(random_digest(), random_tx_id());

        let expected = random_tx_id();
        uut.insert(random_digest(), expected);

        let parent = uut.insert(random_digest(), random_tx_id());

        assert_eq!(parent, Some(expected));
    }

    #[test]
    fn empty_account_has_no_parent() {
        let mut uut = InflightAccountState::default();
        let parent = uut.insert(random_digest(), random_tx_id());

        assert!(parent.is_none());
    }

    #[test]
    fn fully_committed_account_has_no_parent() {
        let mut uut = InflightAccountState::default();
        uut.insert(random_digest(), random_tx_id());
        uut.commit(1);
        let parent = uut.insert(random_digest(), random_tx_id());

        assert!(parent.is_none());
    }

    #[test]
    fn uncommitted_account_has_a_parent() {
        let expected_parent = random_tx_id();

        let mut uut = InflightAccountState::default();
        uut.insert(random_digest(), expected_parent);

        let parent = uut.insert(random_digest(), random_tx_id());

        assert_eq!(parent, Some(expected_parent));
    }

    #[test]
    fn partially_committed_account_has_a_parent() {
        let expected_parent = random_tx_id();

        let mut uut = InflightAccountState::default();
        uut.insert(random_digest(), random_tx_id());
        uut.insert(random_digest(), expected_parent);
        uut.commit(1);

        let parent = uut.insert(random_digest(), random_tx_id());

        assert_eq!(parent, Some(expected_parent));
    }

    #[test]
    fn reverted_txs_are_nonextant() {
        const N: usize = 5;
        const REVERT: usize = 2;

        let states = (0..N).map(|_| (random_digest(), random_tx_id())).collect::<Vec<_>>();

        let mut uut = InflightAccountState::default();
        for (state, tx) in &states {
            uut.insert(*state, *tx);
        }
        uut.revert(REVERT);

        let mut expected = InflightAccountState::default();
        for (state, tx) in states.iter().rev().skip(REVERT).rev() {
            expected.insert(*state, *tx);
        }

        assert_eq!(uut, expected);
    }

    #[test]
    fn pruned_txs_are_nonextant() {
        const N: usize = 5;
        const PRUNE: usize = 2;

        let states = (0..N).map(|_| (random_digest(), random_tx_id())).collect::<Vec<_>>();

        let mut uut = InflightAccountState::default();
        for (state, tx) in &states {
            uut.insert(*state, *tx);
        }
        uut.commit(PRUNE);
        uut.prune_commited(PRUNE);

        let mut expected = InflightAccountState::default();
        for (state, tx) in states.iter().skip(PRUNE) {
            expected.insert(*state, *tx);
        }

        assert_eq!(uut, expected);
    }

    #[test]
    fn is_empty_after_full_commit_and_prune() {
        const N: usize = 5;
        let mut uut = InflightAccountState::default();
        for _ in 0..N {
            uut.insert(random_digest(), random_tx_id());
        }

        uut.commit(N);
        uut.prune_commited(N);

        assert_eq!(uut, Default::default());
    }

    #[test]
    fn is_empty_after_full_revert() {
        const N: usize = 5;
        let mut uut = InflightAccountState::default();
        for _ in 0..N {
            uut.insert(random_digest(), random_tx_id());
        }

        uut.revert(N);

        assert_eq!(uut, Default::default());
    }

    #[test]
    #[should_panic]
    fn revert_panics_on_out_of_bounds() {
        const N: usize = 5;
        let mut uut = InflightAccountState::default();
        for _ in 0..N {
            uut.insert(random_digest(), random_tx_id());
        }

        uut.commit(1);
        uut.revert(N);
    }

    #[test]
    #[should_panic]
    fn commit_panics_on_out_of_bounds() {
        const N: usize = 5;
        let mut uut = InflightAccountState::default();
        for _ in 0..N {
            uut.insert(random_digest(), random_tx_id());
        }

        uut.commit(N + 1);
    }
}
