use std::{path::Path, sync::Arc, time::Duration};

use anyhow::Result;

use batch_builder::batch::TransactionBatch;
use miden_node_utils::config::load_config;
use miden_objects::transaction::ProvenTransaction;
use tokio::sync::RwLock;

#[cfg(test)]
pub mod test_utils;

mod batch_builder;
mod block_builder;
mod errors;
mod state_view;
mod store;
mod txqueue;

use config::BlockProducerTopLevelConfig;

pub mod block;
pub mod config;
pub mod server;

// TYPE ALIASES
// =================================================================================================

/// A proven transaction that can be shared across threads
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

// MAIN FUNCTION
// =================================================================================================

pub async fn start_block_producer(config_filepath: &Path) -> Result<()> {
    // miden_node_utils::logging::setup_logging()?;

    let config: BlockProducerTopLevelConfig = load_config(config_filepath).extract()?;

    server::serve(config.block_producer).await?;

    Ok(())
}
