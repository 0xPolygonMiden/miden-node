use std::sync::Arc;

use miden_objects::{
    accounts::AccountId,
    crypto::rand::{FeltRng, RpoRandomCoin},
    transaction::TransactionId,
    Digest,
};

mod proven_tx;

pub use proven_tx::{mock_proven_tx, MockProvenTxBuilder};

mod store;

use rand::Rng;
pub use store::{MockStoreFailure, MockStoreSuccess, MockStoreSuccessBuilder};

mod account;

pub use account::{mock_account_id, MockPrivateAccount};

pub mod block;

pub mod batch;

pub mod note;

/// Generates random values for tests.
///
/// It prints its seed on construction which allows us to reproduce
/// test failures.
pub struct Random(RpoRandomCoin);

impl Random {
    /// Creates a [Random] with a random seed. This seed is logged
    /// so that it is known for test failures.
    pub fn with_random_seed() -> Self {
        let seed: [u32; 4] = rand::random();

        println!("Random::with_random_seed: {seed:?}");

        let seed = Digest::from(seed).into();

        Self(RpoRandomCoin::new(seed))
    }

    pub fn draw_tx_id(&mut self) -> TransactionId {
        self.0.draw_word().into()
    }

    pub fn draw_digest(&mut self) -> Digest {
        self.0.draw_word().into()
    }
}
