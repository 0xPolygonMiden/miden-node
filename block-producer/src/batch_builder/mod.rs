use std::{cmp::min, sync::Arc, time::Duration};

use async_trait::async_trait;
use tokio::{sync::RwLock, time};
use tracing::info;

use self::errors::BuildBatchError;
use crate::{block_builder::BlockBuilder, SharedProvenTx, SharedRwVec, SharedTxBatch, COMPONENT};

pub mod errors;
#[cfg(test)]
mod tests;

mod batch;
pub use batch::TransactionBatch;

// BATCH BUILDER
// ================================================================================================

#[async_trait]
pub trait BatchBuilder: Send + Sync + 'static {
    /// TODO: add doc comments
    async fn build_batch(
        &self,
        txs: Vec<SharedProvenTx>, 
    ) -> Result<(), BuildBatchError>;
}

// DEFAULT BATCH BUILDER
// ================================================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultBatchBuilderOptions {
    /// The frequency at which blocks are created
    pub block_frequency: Duration,

    /// Maximum number of batches in any given block
    pub max_batches_per_block: usize,
}

pub struct DefaultBatchBuilder<BB> {
    /// Batches ready to be included in a block
    ready_batches: SharedRwVec<SharedTxBatch>,

    block_builder: Arc<BB>,

    options: DefaultBatchBuilderOptions,
}

impl<BB> DefaultBatchBuilder<BB>
where
    BB: BlockBuilder,
{
    // CONSTRUCTOR
    // --------------------------------------------------------------------------------------------
    /// Returns an new [BatchBuilder] instantiated with the provided [BlockBuilder] and the
    /// specified options.
    pub fn new(
        block_builder: Arc<BB>,
        options: DefaultBatchBuilderOptions,
    ) -> Self {
        Self {
            ready_batches: Arc::new(RwLock::new(Vec::new())),
            block_builder,
            options,
        }
    }

    // BATCH BUILDER STARTER
    // --------------------------------------------------------------------------------------------

    /// TODO: add comments
    pub async fn run(self: Arc<Self>) {
        let mut interval = time::interval(self.options.block_frequency);

        loop {
            interval.tick().await;
            self.try_build_block().await;
        }
    }

    // HELPER METHODS
    // --------------------------------------------------------------------------------------------

    /// Note that we call `build_block()` regardless of whether the `ready_batches` queue is empty.
    /// A call to an empty `build_block()` indicates that an empty block should be created.
    async fn try_build_block(&self) {
        let mut batches_in_block: Vec<SharedTxBatch> = {
            let mut locked_ready_batches = self.ready_batches.write().await;

            let num_batches_in_block =
                min(self.options.max_batches_per_block, locked_ready_batches.len());

            locked_ready_batches.drain(..num_batches_in_block).collect()
        };

        match self.block_builder.build_block(batches_in_block.clone()).await {
            Ok(_) => {
                // block successfully built, do nothing
            },
            Err(_) => {
                // Block building failed; add back the batches at the end of the queue
                self.ready_batches.write().await.append(&mut batches_in_block);
            },
        }
    }
}

#[async_trait]
impl<BB> BatchBuilder for DefaultBatchBuilder<BB>
where
    BB: BlockBuilder,
{
    async fn build_batch(
        &self,
        txs: Vec<SharedProvenTx>,
    ) -> Result<(), BuildBatchError> {
        let num_txs = txs.len();

        let batch = Arc::new(TransactionBatch::new(txs)?);
        self.ready_batches.write().await.push(batch);

        info!(COMPONENT, "batch built with {num_txs} txs");

        Ok(())
    }
}
