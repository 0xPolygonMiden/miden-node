use async_trait::async_trait;
use miden_objects::{accounts::AccountId, Digest, Felt};

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

#[derive(Debug)]
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

        let updated_accounts: Vec<(AccountId, Digest)> =
            batches.iter().map(|batch| batch.updated_accounts()).flatten().collect();
        let created_notes: Vec<Digest> =
            batches.iter().map(|batch| batch.created_notes()).flatten().collect();
        let produced_nullifiers: Vec<Digest> =
            batches.iter().map(|batch| batch.produced_nullifiers()).flatten().collect();

        todo!()
    }
}
