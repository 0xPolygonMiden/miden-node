use std::{cmp::min, collections::BTreeSet, sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::{
    notes::NoteId,
    transaction::{InputNoteCommitment, OutputNote},
};
use tokio::time;
use tracing::{debug, info, instrument, Span};

use crate::{block_builder::BlockBuilder, ProvenTransaction, SharedRwVec, COMPONENT};

#[cfg(test)]
mod tests;

pub mod batch;
pub use batch::TransactionBatch;
use miden_node_utils::formatting::{format_array, format_blake3_digest};

use crate::{errors::BuildBatchError, store::Store};

// BATCH BUILDER
// ================================================================================================

/// Abstraction over batch proving of transactions.
///
/// Transactions are aggregated into batches prior to being added to blocks. This trait abstracts
/// over this responsibility. The trait's goal is to be implementation agnostic, allowing for
/// multiple implementations, e.g.:
///
/// - in-process cpu based prover
/// - out-of-process gpu based prover
/// - distributed prover on another machine
#[async_trait]
pub trait BatchBuilder: Send + Sync + 'static {
    /// Start proving of a new batch.
    async fn build_batch(&self, txs: Vec<ProvenTransaction>) -> Result<(), BuildBatchError>;
}

// DEFAULT BATCH BUILDER
// ================================================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultBatchBuilderOptions {
    /// The frequency at which blocks are created
    pub block_frequency: Duration,

    /// Maximum number of batches in any given block
    pub max_batches_per_block: usize,
}

pub struct DefaultBatchBuilder<S, BB> {
    store: Arc<S>,

    block_builder: Arc<BB>,

    options: DefaultBatchBuilderOptions,

    /// Batches ready to be included in a block
    ready_batches: SharedRwVec<TransactionBatch>,
}

// FIXME: remove the allow when the upstream clippy issue is fixed:
// https://github.com/rust-lang/rust-clippy/issues/12281
#[allow(clippy::blocks_in_conditions)]
impl<S, BB> DefaultBatchBuilder<S, BB>
where
    S: Store,
    BB: BlockBuilder,
{
    // CONSTRUCTOR
    // --------------------------------------------------------------------------------------------
    /// Returns an new [BatchBuilder] instantiated with the provided [BlockBuilder] and the
    /// specified options.
    pub fn new(store: Arc<S>, block_builder: Arc<BB>, options: DefaultBatchBuilderOptions) -> Self {
        Self {
            store,
            block_builder,
            options,
            ready_batches: Default::default(),
        }
    }

    // BATCH BUILDER STARTER
    // --------------------------------------------------------------------------------------------
    pub async fn run(self: Arc<Self>) {
        let mut interval = time::interval(self.options.block_frequency);

        info!(target: COMPONENT, period_ms = interval.period().as_millis(), "Batch builder started");

        loop {
            interval.tick().await;
            self.try_build_block().await;
        }
    }

    // HELPER METHODS
    // --------------------------------------------------------------------------------------------

    /// Note that we call `build_block()` regardless of whether the `ready_batches` queue is empty.
    /// A call to an empty `build_block()` indicates that an empty block should be created.
    #[instrument(target = "miden-block-producer", skip_all)]
    async fn try_build_block(&self) {
        let mut batches_in_block: Vec<TransactionBatch> = {
            let mut locked_ready_batches = self.ready_batches.write().await;

            let num_batches_in_block =
                min(self.options.max_batches_per_block, locked_ready_batches.len());

            locked_ready_batches.drain(..num_batches_in_block).collect()
        };

        match self.block_builder.build_block(&batches_in_block).await {
            Ok(_) => {
                // block successfully built, do nothing
            },
            Err(_) => {
                // Block building failed; add back the batches at the end of the queue
                self.ready_batches.write().await.append(&mut batches_in_block);
            },
        }
    }

    async fn find_dangling_notes(&self, txs: &[ProvenTransaction]) -> Vec<NoteId> {
        // TODO: We can optimize this by looking at the notes created in the previous batches
        let note_created: BTreeSet<NoteId> = txs
            .iter()
            .flat_map(|tx| tx.output_notes().iter().map(OutputNote::id))
            .chain(
                self.ready_batches
                    .read()
                    .await
                    .iter()
                    .flat_map(|batch| batch.created_notes().iter().map(OutputNote::id)),
            )
            .collect();

        txs.iter()
            .flat_map(|tx| {
                tx.input_notes()
                    .iter()
                    .filter_map(InputNoteCommitment::note_id)
                    .filter(|note_id| !note_created.contains(note_id))
            })
            .collect()
    }
}

// FIXME: remove the allow when the upstream clippy issue is fixed:
// https://github.com/rust-lang/rust-clippy/issues/12281
#[allow(clippy::blocks_in_conditions)]
#[async_trait]
impl<S, BB> BatchBuilder for DefaultBatchBuilder<S, BB>
where
    S: Store,
    BB: BlockBuilder,
{
    #[instrument(target = "miden-block-producer", skip_all, err, fields(batch_id))]
    async fn build_batch(&self, txs: Vec<ProvenTransaction>) -> Result<(), BuildBatchError> {
        let num_txs = txs.len();

        info!(target: COMPONENT, num_txs, "Building a transaction batch");
        debug!(target: COMPONENT, txs = %format_array(txs.iter().map(|tx| tx.id().to_hex())));

        let dangling_notes = self.find_dangling_notes(&txs).await;
        if !dangling_notes.is_empty() {
            let missing_notes = match self.store.get_missing_notes(&dangling_notes).await {
                Ok(notes) => notes,
                Err(err) => return Err(BuildBatchError::GetMissingNotesRequestError(err, txs)),
            };

            if !missing_notes.is_empty() {
                return Err(BuildBatchError::FutureNotesNotFound(missing_notes, txs));
            }
        }

        let batch = TransactionBatch::new(txs)?;

        info!(target: COMPONENT, "Transaction batch built");
        Span::current().record("batch_id", format_blake3_digest(batch.id()));

        let num_batches = {
            let mut write_guard = self.ready_batches.write().await;
            write_guard.push(batch);
            write_guard.len()
        };

        info!(target: COMPONENT, num_batches, "Transaction batch added to the batch queue");

        Ok(())
    }
}
