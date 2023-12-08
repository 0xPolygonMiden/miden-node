use std::sync::Arc;

use async_trait::async_trait;
use miden_objects::{accounts::AccountId, Digest};

use crate::{
    batch_builder::MAX_NUM_CREATED_NOTES_PER_BATCH, block::Block, store::Store, SharedTxBatch,
};

pub mod errors;

pub(crate) mod prover;
use self::{
    errors::BuildBlockError,
    prover::{block_witness::BlockWitness, BlockProver},
};

#[cfg(test)]
mod tests;

// BLOCK BUILDER
// =================================================================================================

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
    block_kernel: BlockProver,
}

impl<S> DefaultBlockBuilder<S>
where
    S: Store,
{
    pub fn new(store: Arc<S>) -> Self {
        Self {
            store,
            block_kernel: BlockProver::new(),
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
        let account_updates: Vec<(AccountId, Digest)> =
            batches.iter().flat_map(|batch| batch.updated_accounts()).collect();
        let created_notes = batches
            .iter()
            .enumerate()
            .flat_map(|(batch_idx, batch)| {
                batch.created_notes().enumerate().map(move |(note_idx_in_batch, note)| {
                    let note_idx_in_block =
                        batch_idx * MAX_NUM_CREATED_NOTES_PER_BATCH + note_idx_in_batch;
                    (note_idx_in_block as u64, *note)
                })
            })
            .collect();
        let produced_nullifiers: Vec<Digest> =
            batches.iter().flat_map(|batch| batch.produced_nullifiers()).collect();

        let block_inputs = self
            .store
            .get_block_inputs(
                account_updates.iter().map(|(account_id, _)| account_id),
                produced_nullifiers.iter(),
            )
            .await?;

        let block_header_witness = BlockWitness::new(block_inputs, batches)?;

        let new_block_header = self.block_kernel.prove(block_header_witness)?;

        let block = Arc::new(Block {
            header: new_block_header,
            updated_accounts: account_updates,
            created_notes,
            produced_nullifiers,
        });

        self.store.apply_block(block.clone()).await?;

        Ok(())
    }
}
