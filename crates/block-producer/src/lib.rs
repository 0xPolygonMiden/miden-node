// TODO: remove once block-producer rework is complete
#![allow(unused)]

use std::{sync::Arc, time::Duration};

use batch_builder::batch::TransactionBatch;
use miden_objects::transaction::ProvenTransaction;
use tokio::sync::RwLock;

#[cfg(test)]
pub mod test_utils;

mod batch_builder;
mod block_builder;
mod errors;
mod mempool;
mod state_view;
mod store;
mod txqueue;

pub mod block;
pub mod config;
pub mod server;

// TYPE ALIASES
// =================================================================================================

/// A vector that can be shared across threads
pub(crate) type SharedRwVec<T> = Arc<RwLock<Vec<T>>>;

// CONSTANTS
// =================================================================================================

/// The name of the block producer component
pub const COMPONENT: &str = "miden-block-producer";

/// The number of transactions per batch
const SERVER_BATCH_SIZE: usize = 2;

/// The frequency at which blocks are produced
const SERVER_BLOCK_FREQUENCY: Duration = Duration::from_secs(10);

/// The frequency at which batches are built
const SERVER_BUILD_BATCH_FREQUENCY: Duration = Duration::from_secs(2);

/// Maximum number of batches per block
const SERVER_MAX_BATCHES_PER_BLOCK: usize = 4;
