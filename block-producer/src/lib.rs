use std::sync::Arc;

use miden_objects::transaction::ProvenTransaction;

#[cfg(test)]
pub mod test_utils;

pub mod batch_builder;
pub mod block_builder;
pub mod txqueue;

/// A proven transaction that can be shared across threads
pub(crate) type SharedProvenTx = Arc<ProvenTransaction>;
