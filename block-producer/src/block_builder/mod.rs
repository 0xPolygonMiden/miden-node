use std::{fmt::Debug, sync::Arc};

use async_trait::async_trait;

use crate::batch_builder::TransactionBatch;

#[derive(Debug)]
pub enum AddBatchesError {}

#[async_trait]
pub trait BlockBuilder: Send + Sync + 'static {
    /// Receive batches to be included in a block.
    ///
    /// The `BlockBuilder` relies on `add_batches()` to be called as a precondition to creating a
    /// block. In other words, if `add_batches()` is never called, then no blocks are produced.
    async fn add_batches(
        &self,
        batches: Vec<Arc<TransactionBatch>>,
    ) -> Result<(), AddBatchesError>;
}
