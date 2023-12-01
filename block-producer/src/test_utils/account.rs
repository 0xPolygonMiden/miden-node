use miden_objects::{accounts::get_account_seed, Hasher};

use super::*;

/// A mock representation fo private accounts. An account starts in state `states[0]`, is modified
/// to state `states[1]`, and so on.
#[derive(Clone, Copy, Debug)]
pub struct MockPrivateAccount<const NUM_STATES: usize = 3> {
    pub id: AccountId,

    // Sequence states that the account goes into.
    pub states: [Digest; NUM_STATES],
}

impl<const NUM_STATES: usize> MockPrivateAccount<NUM_STATES> {
    fn new(init_seed: [u8; 32]) -> Self {
        let account_seed = get_account_seed(
            init_seed,
            miden_objects::accounts::AccountType::RegularAccountUpdatableCode,
            false,
            Digest::default(),
            Digest::default(),
        )
        .unwrap();

        let mut states = [Digest::default(); NUM_STATES];

        states[0] = Hasher::hash(&init_seed);
        for idx in 1..NUM_STATES {
            states[idx] = Hasher::hash(&states[idx - 1].as_bytes());
        }

        Self {
            id: AccountId::new(account_seed, Digest::default(), Digest::default()).unwrap(),
            states,
        }
    }
}

impl<const NUM_STATES: usize> From<u32> for MockPrivateAccount<NUM_STATES> {
    /// Each index gives rise to a different account ID
    fn from(index: u32) -> Self {
        let init_seed: Vec<_> = index.to_be_bytes().into_iter().chain([0u8; 28]).collect();

        Self::new(init_seed.try_into().unwrap())
    }
}
