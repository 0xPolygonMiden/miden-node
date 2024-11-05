use std::time::Duration;

#[cfg(test)]
pub mod test_utils;

mod batch_builder;
mod block_builder;
mod domain;
mod errors;
mod mempool;
mod store;

pub mod block;
pub mod config;
pub mod server;

// CONSTANTS
// =================================================================================================

/// The name of the block producer component
pub const COMPONENT: &str = "miden-block-producer";

/// The number of transactions per batch
const SERVER_MAX_TXS_PER_BATCH: usize = 2;

/// The frequency at which blocks are produced
const SERVER_BLOCK_FREQUENCY: Duration = Duration::from_secs(10);

/// The frequency at which batches are built
const SERVER_BUILD_BATCH_FREQUENCY: Duration = Duration::from_secs(2);

/// Maximum number of batches per block
const SERVER_MAX_BATCHES_PER_BLOCK: usize = 4;

/// The number of blocks of committed state that the mempool retains.
///
/// This determines the grace period incoming transactions have between fetching their input from
/// the store and verification in the mempool.
const SERVER_MEMPOOL_STATE_RETENTION: usize = 5;
