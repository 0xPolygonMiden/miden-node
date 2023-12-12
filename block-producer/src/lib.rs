use std::{sync::Arc, time::Duration};

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

/// The name of the block producer component
pub const COMPONENT: &str = "miden-block-producer";

/// The depth of the SMT for created notes
pub(crate) const CREATED_NOTES_SMT_DEPTH: u8 = 13;

/// The maximum number of created notes per batch.
///
/// The created notes tree uses an extra depth to store the 2 components of `NoteEnvelope`.
/// That is, conceptually, notes sit at depth 12; where in reality, depth 12 contains the
/// hash of level 13, where both the `note_hash()` and metadata are stored (one per node).
pub(crate) const MAX_NUM_CREATED_NOTES_PER_BATCH: usize =
    2_usize.pow((CREATED_NOTES_SMT_DEPTH - 1) as u32);

/// The number of transactions per batch
pub(crate) const SERVER_BATCH_SIZE: usize = 2;

/// The frequency at which blocks are produced
pub(crate) const SERVER_BLOCK_FREQUENCY: Duration = Duration::from_secs(10);

/// The frequency at which batches are built
pub(crate) const SERVER_BUILD_BATCH_FREQUENCY: Duration = Duration::from_secs(2);

/// Maximum number of batches per block
pub(crate) const SERVER_MAX_BATCHES_PER_BLOCK: usize = 4;
