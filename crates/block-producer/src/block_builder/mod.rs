use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use async_trait::async_trait;
use miden_node_utils::formatting::{format_array, format_blake3_digest};
use miden_objects::{
    accounts::AccountId,
    block::Block,
    notes::{NoteHeader, Nullifier},
    transaction::InputNoteCommitment,
};
use tokio::sync::Mutex;
use tracing::{debug, info, instrument};

use crate::{
    batch_builder::batch::TransactionBatch,
    errors::BuildBlockError,
    mempool::{BatchJobId, Mempool},
    store::{ApplyBlock, DefaultStore, Store},
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

        let updated_account_set: BTreeSet<AccountId> = batches
            .iter()
            .flat_map(TransactionBatch::updated_accounts)
            .map(|(account_id, _)| *account_id)
            .collect();

        let output_notes: Vec<_> =
            batches.iter().map(TransactionBatch::output_notes).cloned().collect();

        let produced_nullifiers: Vec<Nullifier> =
            batches.iter().flat_map(TransactionBatch::produced_nullifiers).collect();

        // Populate set of output notes from all batches
        let output_notes_set: BTreeSet<_> = output_notes
            .iter()
            .flat_map(|batch| batch.iter().map(|note| note.id()))
            .collect();

        // Build a set of unauthenticated input notes for this block which do not have a matching
        // output note produced in this block
        let dangling_notes: BTreeSet<_> = batches
            .iter()
            .flat_map(TransactionBatch::input_notes)
            .filter_map(InputNoteCommitment::header)
            .map(NoteHeader::id)
            .filter(|note_id| !output_notes_set.contains(note_id))
            .collect();

        // Request information needed for block building from the store
        let block_inputs = self
            .store
            .get_block_inputs(
                updated_account_set.into_iter(),
                produced_nullifiers.iter(),
                dangling_notes.iter(),
            )
            .await?;

        let missing_notes: Vec<_> = dangling_notes
            .difference(&block_inputs.found_unauthenticated_notes.note_ids())
            .copied()
            .collect();
        if !missing_notes.is_empty() {
            return Err(BuildBlockError::UnauthenticatedNotesNotFound(missing_notes));
        }

        let (block_header_witness, updated_accounts) = BlockWitness::new(block_inputs, batches)?;

        let new_block_header = self.block_kernel.prove(block_header_witness)?;
        let block_num = new_block_header.block_num();

        // TODO: return an error?
        let block =
            Block::new(new_block_header, updated_accounts, output_notes, produced_nullifiers)
                .expect("invalid block components");

        let block_hash = block.hash();

        info!(target: COMPONENT, block_num, %block_hash, "block built");
        debug!(target: COMPONENT, ?block);

        self.state_view.apply_block(&block).await?;

        info!(target: COMPONENT, block_num, %block_hash, "block committed");

        Ok(())
    }
}

struct BlockProducer<BB> {
    pub mempool: Arc<Mutex<Mempool>>,
    pub block_interval: tokio::time::Duration,
    pub block_builder: BB,
}

impl<BB: BlockBuilder> BlockProducer<BB> {
    pub async fn run(self) {
        let mut interval = tokio::time::interval(self.block_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            let (block_number, batches) = self.mempool.lock().await.select_block();
            let batches = batches.into_values().collect::<Vec<_>>();

            let result = self.block_builder.build_block(&batches).await;
            let mut mempool = self.mempool.lock().await;

            match result {
                Ok(_) => mempool.block_committed(block_number),
                Err(_) => mempool.block_failed(block_number),
            }
        }
    }
}
