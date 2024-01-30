use std::{cmp::min, sync::Arc, time::Duration};

use async_trait::async_trait;
use tokio::{sync::RwLock, time};
use tracing::{debug, info, instrument, Span};

use self::errors::BuildBatchError;
use crate::{block_builder::BlockBuilder, SharedProvenTx, SharedRwVec, SharedTxBatch, COMPONENT};

pub mod errors;
#[cfg(test)]
mod tests;

mod batch;
pub use batch::TransactionBatch;
use miden_node_utils::logging::{format_array, format_blake3_digest};

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

        info!(target: COMPONENT, period_ms = interval.period().as_millis(), "Batch builder started");

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
    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(target = "miden-block-producer", skip_all, err, fields(batch_id))]
    async fn build_batch(
        &self,
        txs: Vec<SharedProvenTx>,
    ) -> Result<(), BuildBatchError> {
        let num_txs = txs.len();

        info!(target: COMPONENT, num_txs, "Building a transaction batch");
        debug!(target: COMPONENT, txs = %format_array(txs.iter().map(|tx| tx.id().to_hex())));

        let batch = Arc::new(TransactionBatch::new(txs)?);

        info!(target: COMPONENT, "Transaction batch built");
        Span::current().record("batch_id", format_blake3_digest(batch.id()));

        let num_batches = {
            let mut write_guard = self.ready_batches.write().await;
            write_guard.push(batch);
            write_guard.len()
        };

        info!(target: COMPONENT, num_batches, "Transaction batch added to the batch queue");

        Ok(())
    }
}
