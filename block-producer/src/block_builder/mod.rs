use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use miden_objects::{accounts::AccountId, BlockHeader, Digest, Felt};
use tokio::sync::RwLock;

use crate::{block::Block, store::Store, SharedTxBatch};

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
    store: Arc<S>,
    protocol_version: Felt,
    /// The hash of the previous header
    prev_header_hash: Arc<RwLock<Digest>>,
    /// The hash of the previous block
    prev_block_hash: Arc<RwLock<Digest>>,
    /// The block number of the next block to build
    next_block_num: Arc<RwLock<Felt>>,
}

impl<S> DefaultBlockBuilder<S>
where
    S: Store,
{
    pub fn new(
        store: Arc<S>,
        protocol_version: Felt,
        prev_header_hash: Digest,
        prev_block_hash: Digest,
        prev_block_num: Felt,
    ) -> Self {
        Self {
            store,
            protocol_version,
            prev_header_hash: Arc::new(RwLock::new(prev_header_hash)),
            prev_block_hash: Arc::new(RwLock::new(prev_block_hash)),
            next_block_num: Arc::new(RwLock::new(prev_block_num + 1u32.into())),
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
        let current_block_num = *self.next_block_num.read().await;

        let updated_accounts: Vec<(AccountId, Digest)> =
            batches.iter().map(|batch| batch.updated_accounts()).flatten().collect();
        let created_notes: Vec<Digest> =
            batches.iter().map(|batch| batch.created_notes()).flatten().collect();
        let produced_nullifiers: Vec<Digest> =
            batches.iter().map(|batch| batch.produced_nullifiers()).flatten().collect();

        let header = {
            let prev_hash = *self.prev_header_hash.read().await;
            let chain_root = Digest::default();
            let account_root = Digest::default();
            let nullifier_root = Digest::default();
            let note_root = Digest::default();
            let batch_root = Digest::default();
            let proof_hash = Digest::default();
            let timestamp: Felt = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("today is expected to be before 1970")
                .as_millis()
                .into();

            BlockHeader::new(
                prev_hash,
                current_block_num,
                chain_root,
                account_root,
                nullifier_root,
                note_root,
                batch_root,
                proof_hash,
                self.protocol_version,
                timestamp,
            )
        };

        let block = Arc::new(Block {
            header,
            updated_accounts,
            created_notes,
            produced_nullifiers,
        });

        // TODO: properly handle
        self.store.apply_block(block.clone()).await.expect("apply block failed");

        // update fields
        *self.prev_header_hash.write().await = block.header.hash();
        *self.prev_block_hash.write().await = block.hash();
        *self.next_block_num.write().await = current_block_num + 1u32.into();

        Ok(())
    }
}
