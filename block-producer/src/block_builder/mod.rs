use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use miden_objects::{accounts::AccountId, BlockHeader, Digest, Felt};

use crate::{
    block::Block,
    store::{BlockInputs, Store},
    SharedTxBatch,
};

mod account;

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
}

impl<S> DefaultBlockBuilder<S>
where
    S: Store,
{
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
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
        let updated_accounts: Vec<(AccountId, Digest)> =
            batches.iter().flat_map(|batch| batch.updated_accounts()).collect();
        let created_notes: Vec<Digest> =
            batches.iter().flat_map(|batch| batch.created_notes()).collect();
        let produced_nullifiers: Vec<Digest> =
            batches.iter().flat_map(|batch| batch.produced_nullifiers()).collect();

        let BlockInputs {
            block_header: prev_block_header,
            chain_peaks,
            account_states: account_states_in_store,
            nullifiers,
        } = self
            .store
            .get_block_inputs(
                updated_accounts.iter().map(|(account_id, _)| account_id),
                produced_nullifiers.iter(),
            )
            .await
            .unwrap();

        let new_block_header = {
            let prev_hash = prev_block_header.prev_hash();
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
                prev_block_header.block_num(),
                chain_root,
                account_root,
                nullifier_root,
                note_root,
                batch_root,
                proof_hash,
                prev_block_header.version(),
                timestamp,
            )
        };

        let block = Arc::new(Block {
            header: new_block_header,
            updated_accounts,
            created_notes,
            produced_nullifiers,
        });

        // TODO: properly handle
        self.store.apply_block(block.clone()).await.expect("apply block failed");

        Ok(())
    }
}
