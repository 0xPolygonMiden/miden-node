use std::sync::Arc;

use async_trait::async_trait;
use miden_objects::{accounts::AccountId, Digest};
use tracing::{info, instrument};

use crate::{
    block::Block,
    store::{ApplyBlock, Store},
    SharedTxBatch, COMPONENT, MAX_NUM_CREATED_NOTES_PER_BATCH,
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
pub struct DefaultBlockBuilder<S, A> {
    store: Arc<S>,
    state_view: Arc<A>,
    block_kernel: BlockProver,
}

impl<S, A> DefaultBlockBuilder<S, A>
where
    S: Store,
    A: ApplyBlock,
{
    pub fn new(
        store: Arc<S>,
        state_view: Arc<A>,
    ) -> Self {
        Self {
            store,
            state_view,
            block_kernel: BlockProver::new(),
        }
    }
}

#[async_trait]
impl<S, A> BlockBuilder for DefaultBlockBuilder<S, A>
where
    S: Store,
    A: ApplyBlock,
{
    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(skip(self), ret, err, fields(COMPONENT))]
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

        let block_num = new_block_header.block_num();
        let block_hash = new_block_header.hash();

        info!(
            prev_hash = %new_block_header.prev_hash(),
            block_num,
            chain_root = %new_block_header.chain_root(),
            account_root = %new_block_header.account_root(),
            nullifier_root = %new_block_header.nullifier_root(),
            note_root = %new_block_header.note_root(),
            batch_root = %new_block_header.batch_root(),
            proof_hash = %new_block_header.proof_hash(),
            version = %new_block_header.version(),
            timestamp = %new_block_header.timestamp(),
            sub_hash = %new_block_header.sub_hash(),
            %block_hash,
            COMPONENT,
            "New block header",
        );

        let block = Block {
            header: new_block_header,
            updated_accounts: account_updates,
            created_notes,
            produced_nullifiers,
        };

        self.state_view.apply_block(block).await?;

        info!(block_num, %block_hash, COMPONENT, "block has been built!");

        Ok(())
    }
}
