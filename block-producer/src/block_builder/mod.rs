use async_trait::async_trait;
use miden_objects::{Digest, Felt};

use crate::{store::Store, SharedTxBatch};

#[derive(Debug, PartialEq)]
pub enum BuildBlockError {
    Dummy,
}

#[async_trait]
pub trait BlockBuilder: Send + Sync + 'static {
    /// Receive batches to be included in a block. An empty vector indicates that no batches were
    /// ready, and that an empty block should be created.
    ///
    /// The `BlockBuilder` relies on `build_block()` to be called as a precondition to creating a
    /// block. In other words, if `build_block()` is never called, then no blocks are produced.
    async fn build_block(
        &self,
        batches: Vec<SharedTxBatch>,
    ) -> Result<(), BuildBlockError>;
}

pub struct DefaultBlockBuilder<S> {
    store: S,
    prev_header_hash: Digest,
    prev_block_hash: Digest,
    prev_block_num: Felt,
}

impl<S> DefaultBlockBuilder<S>
where
    S: Store,
{
    pub fn new(
        store: S,
        prev_header_hash: Digest,
        prev_block_hash: Digest,
        prev_block_num: Felt,
    ) -> Self {
        Self {
            store,
            prev_header_hash,
            prev_block_hash,
            prev_block_num,
        }
    }
}

#[async_trait]
impl<S> BlockBuilder for DefaultBlockBuilder<S>
where
    S: Store,
{
    async fn build_block(
        &self,
        batches: Vec<SharedTxBatch>,
    ) -> Result<(), BuildBlockError> {
        // TODO: call store.get_block_inputs()

        todo!()
    }
}
