use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use miden_node_utils::formatting::{format_array, format_blake3_digest};
use miden_objects::{
    block::{Block, BlockAccountUpdate},
    notes::Nullifier,
};
use tracing::{debug, info, instrument};

use crate::{
    batch_builder::batch::TransactionBatch,
    errors::BuildBlockError,
    store::{ApplyBlock, Store},
    COMPONENT,
};

pub(crate) mod prover;

use self::prover::{block_witness::BlockWitness, BlockProver};

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
    async fn build_block(&self, batches: &[TransactionBatch]) -> Result<(), BuildBlockError>;
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
    pub fn new(store: Arc<S>, state_view: Arc<A>) -> Self {
        Self {
            store,
            state_view,
            block_kernel: BlockProver::new(),
        }
    }
}

// FIXME: remove the allow when the upstream clippy issue is fixed:
// https://github.com/rust-lang/rust-clippy/issues/12281
#[allow(clippy::blocks_in_conditions)]
#[async_trait]
impl<S, A> BlockBuilder for DefaultBlockBuilder<S, A>
where
    S: Store,
    A: ApplyBlock,
{
    #[instrument(target = "miden-block-producer", skip_all, err)]
    async fn build_block(&self, batches: &[TransactionBatch]) -> Result<(), BuildBlockError> {
        info!(
            target: COMPONENT,
            num_batches = batches.len(),
            batches = %format_array(batches.iter().map(|batch| format_blake3_digest(batch.id()))),
        );

        let updated_accounts: Vec<_> =
            batches.iter().flat_map(TransactionBatch::updated_accounts).collect();

        let created_notes: Vec<_> =
            batches.iter().map(TransactionBatch::output_notes).cloned().collect();

        let produced_nullifiers: Vec<Nullifier> =
            batches.iter().flat_map(TransactionBatch::produced_nullifiers).collect();

        let created_notes_set: BTreeSet<_> = created_notes
            .iter()
            .flat_map(|batch| batch.iter().map(|note| note.id()))
            .collect();

        let dangling_notes = batches
            .iter()
            .flat_map(TransactionBatch::unauthenticated_input_notes)
            .filter(|&note_id| !created_notes_set.contains(note_id));

        let block_inputs = self
            .store
            .get_block_inputs(
                updated_accounts.iter().map(BlockAccountUpdate::account_id),
                produced_nullifiers.iter(),
                dangling_notes,
            )
            .await?;

        if !block_inputs.missing_notes.is_empty() {
            return Err(BuildBlockError::UnauthenticatedNotesNotFound(block_inputs.missing_notes));
        }

        let block_header_witness = BlockWitness::new(block_inputs, batches)?;

        let new_block_header = self.block_kernel.prove(block_header_witness)?;
        let block_num = new_block_header.block_num();

        // TODO: return an error?
        let block =
            Block::new(new_block_header, updated_accounts, created_notes, produced_nullifiers)
                .expect("invalid block components");

        let block_hash = block.hash();

        info!(target: COMPONENT, block_num, %block_hash, "block built");
        debug!(target: COMPONENT, ?block);

        self.state_view.apply_block(&block).await?;

        info!(target: COMPONENT, block_num, %block_hash, "block committed");

        Ok(())
    }
}
