use std::sync::Arc;

use batch_builder::TransactionBatch;
use miden_objects::transaction::ProvenTransaction;
use tokio::sync::RwLock;

#[cfg(test)]
pub mod test_utils;

mod batch_builder;
mod block_builder;
mod state_view;
mod store;
mod txqueue;

pub mod block;
pub mod cli;
pub mod config;
pub mod server;

// TYPE ALIASES
// =================================================================================================

/// A proven transaction that can be shared across threads
pub(crate) type SharedProvenTx = Arc<ProvenTransaction>;
pub(crate) type SharedTxBatch = Arc<TransactionBatch>;
pub(crate) type SharedRwVec<T> = Arc<RwLock<Vec<T>>>;

// CONSTANTS
// =================================================================================================

pub const COMPONENT: &str = "miden-block-producer";
