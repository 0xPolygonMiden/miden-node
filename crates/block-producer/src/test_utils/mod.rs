use std::sync::Arc;

use miden_objects::{accounts::AccountId, transaction::TransactionId, Digest};

mod proven_tx;

pub use proven_tx::{mock_proven_tx, MockProvenTxBuilder};

mod store;

pub use store::{MockStoreFailure, MockStoreSuccess, MockStoreSuccessBuilder};

mod account;

pub use account::{mock_account_id, MockPrivateAccount};

pub mod block;

pub mod batch;

pub mod note;

/// Generates a [`TransactionId`] from random u32s.
pub fn random_tx_id() -> TransactionId {
    TransactionId::from(random_digest())
}

/// Generates a [`Digest`] from random u32s.
pub fn random_digest() -> Digest {
    let felts: [u32; 4] = rand::random();
    Digest::from(felts)
}
