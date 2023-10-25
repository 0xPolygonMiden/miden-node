use std::{fmt::Debug, sync::Arc};

use async_trait::async_trait;

use crate::{batch_builder::TransactionBatch, SharedTxBatch};

#[derive(Debug)]
pub enum BuildBlockError {
    Dummy,
}

#[async_trait]
pub trait BlockBuilder: Send + Sync + 'static {
    /// Receive batches to be included in a block. `None` indicates that no batches were ready, and
    /// that an empty block should be created.
    ///
    /// The `BlockBuilder` relies on `build_block()` to be called as a precondition to creating a
    /// block. In other words, if `build_block()` is never called, then no blocks are produced.
    async fn build_block(
        &self,
        batches: Option<Vec<SharedTxBatch>>,
    ) -> Result<(), BuildBlockError>;
}
