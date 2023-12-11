use std::sync::Arc;

use batch_builder::TransactionBatch;
use miden_objects::transaction::ProvenTransaction;
use tokio::sync::RwLock;

#[cfg(test)]
pub mod test_utils;

pub mod batch_builder;
pub mod block;
pub mod block_builder;
pub mod cli;
pub mod config;
pub mod constants;
pub mod server;
pub mod state_view;
pub mod store;
pub mod txqueue;

/// A proven transaction that can be shared across threads
pub(crate) type SharedProvenTx = Arc<ProvenTransaction>;
pub(crate) type SharedTxBatch = Arc<TransactionBatch>;
pub(crate) type SharedRwVec<T> = Arc<RwLock<Vec<T>>>;
