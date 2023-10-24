use std::{fmt::Debug, sync::Arc};

use async_trait::async_trait;

use crate::batch_builder::TransactionBatch;

#[derive(Debug)]
pub enum BuildBlockError {}

#[async_trait]
pub trait BlockBuilder: Send + Sync + 'static {
    /// Receive batches to be included in a block.
    ///
    /// The `BlockBuilder` relies on `build_block()` to be called as a precondition to creating a
    /// block. In other words, if `build_block()` is never called, then no blocks are produced.
    async fn build_block(
        &self,
        batches: Vec<Arc<TransactionBatch>>,
    ) -> Result<(), BuildBlockError>;
}
