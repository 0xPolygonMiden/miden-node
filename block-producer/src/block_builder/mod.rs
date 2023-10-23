use std::{fmt::Debug, sync::Arc};

use async_trait::async_trait;

use crate::batch_builder::TransactionBatch;

#[async_trait]
pub trait BlockBuilder: Send + Sync + 'static {
    type AddBatchesError: Debug;

    /// Receive batches to be included in a block.
    /// 
    /// The `BlockBuilder` relies on `add_batches()` to be called as a precondition to creating a
    /// block. In other words, if `add_batches()` is never called, then no blocks are produced.
    fn add_batches(
        &self,
        batches: Vec<Arc<TransactionBatch>>,
    ) -> Result<(), Self::AddBatchesError>;
}
