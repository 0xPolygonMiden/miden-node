use std::collections::{BTreeMap, VecDeque};

use miden_objects::{accounts::AccountId, Digest};

use crate::errors::VerifyTxError;

/// Tracks the list of inflight account updates.
///
/// New transactions can be registered with [Self::verify_and_add]. States that are no longer considered inflight
/// (e.g. due to being applied) may be removed using [Self::remove].
///
/// Both functions perform safety checks to ensure the states match what we expect.
#[derive(Debug, Default)]
pub struct InflightAccountStates(BTreeMap<AccountId, VecDeque<Digest>>);

impl InflightAccountStates {
    /// Verifies that the provided initial state matches the latest inflight account state (if any).
    pub fn verify_update(&self, id: AccountId, init_state: Digest) -> Result<(), VerifyTxError> {
        if let Some(latest) = self.get(id) {
            if latest != &init_state {
                return Err(VerifyTxError::IncorrectAccountInitialHash {
                    tx_initial_account_hash: init_state,
                    actual_account_hash: Some(*latest),
                });
            }
        }

        Ok(())
    }

    /// [Verifies](Self::verify_update) the update and appends it to the list of inflight account
    /// updates.
    pub fn verify_and_add(
        &mut self,
        id: AccountId,
        init_state: Digest,
        final_state: Digest,
    ) -> Result<(), VerifyTxError> {
        let states = self.0.entry(id).or_default();

        // Ensure the latest state matches the new inital state.
        if let Some(latest) = states.back() {
            if latest != &init_state {
                return Err(VerifyTxError::IncorrectAccountInitialHash {
                    tx_initial_account_hash: init_state,
                    actual_account_hash: Some(*latest),
                });
            }
        }

        states.push_back(final_state);

        Ok(())
    }

    /// Remove state transitions from earliest up until a state that matches the given
    /// final state. Returns an error if no match was found.
    ///
    /// In other words, if an account has state transitions `a->b->c->d` then calling `remove(b)`
    /// would leave behind `c->d`.
    pub fn remove(&mut self, id: AccountId, final_state: Digest) -> Result<(), ()> {
        let states = self.0.get_mut(&id).ok_or(())?;
        let Some(idx) = states.iter().position(|x| x == &final_state) else {
            return Err(());
        };

        states.drain(..=idx);
        // Prevent infinite growth by removing entries which have no
        // inflight state changes.
        if states.is_empty() {
            self.0.remove(&id);
        }

        Ok(())
    }

    /// The latest value of the given account.
    pub fn get(&self, id: AccountId) -> Option<&Digest> {
        self.0.get(&id).and_then(|states| states.back())
    }

    /// Number of accounts with inflight transactions.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use miden_air::Felt;
    use miden_objects::accounts::AccountId;

    use super::*;

    #[test]
    fn account_states_must_chain() {
        let account: AccountId = AccountId::new_unchecked(Felt::new(10));
        const ONE: Digest = Digest::new([Felt::new(1), Felt::new(1), Felt::new(1), Felt::new(1)]);
        const TWO: Digest = Digest::new([Felt::new(2), Felt::new(2), Felt::new(2), Felt::new(2)]);
        const THREE: Digest = Digest::new([Felt::new(3), Felt::new(3), Felt::new(3), Felt::new(3)]);
        let mut uut = InflightAccountStates::default();

        assert!(uut.verify_and_add(account, Digest::default(), ONE).is_ok());
        assert!(uut.verify_and_add(account, ONE, TWO).is_ok());
        assert!(uut.verify_and_add(account, TWO, THREE).is_ok());
        assert!(uut.verify_and_add(account, TWO, ONE).is_err());

        assert!(uut.remove(account, TWO).is_ok());
        // Repeat removal should fail since this is no longer present.
        assert!(uut.remove(account, TWO).is_err());
        assert!(uut.remove(account, THREE).is_ok());

        // Check that cleanup is performed.
        assert!(uut.0.is_empty());
    }
}
