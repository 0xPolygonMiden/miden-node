use std::{
    cmp::min,
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    ops::Range,
    sync::Arc,
    time::Duration,
};

use miden_objects::{
    accounts::AccountId,
    assembly::SourceManager,
    notes::NoteId,
    transaction::{OutputNote, TransactionId},
    Digest,
};
use rand::Rng;
use tokio::{sync::Mutex, task::JoinSet, time};
use tonic::async_trait;
use tracing::{debug, info, instrument, Span};

use crate::{
    block_builder::BlockBuilder,
    domain::transaction::AuthenticatedTransaction,
    mempool::{BatchJobId, Mempool},
    ProvenTransaction, SharedRwVec, COMPONENT,
};

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

    /// Returns a list of IDs for unauthenticated notes which are not output notes of any ready
    /// transaction batch or the candidate batch itself.
    async fn find_dangling_notes(&self, txs: &[ProvenTransaction]) -> Vec<NoteId> {
        // TODO: We can optimize this by looking at the notes created in the previous batches

        // build a set of output notes from all ready batches and the candidate batch
        let mut all_output_notes: BTreeSet<NoteId> = txs
            .iter()
            .flat_map(|tx| tx.output_notes().iter().map(OutputNote::id))
            .chain(
                self.ready_batches
                    .read()
                    .await
                    .iter()
                    .flat_map(|batch| batch.output_notes().iter().map(OutputNote::id)),
            )
            .collect();

        // from the list of unauthenticated notes in the candidate batch, filter out any note
        // which is also an output note either in any of the ready batches or in the candidate
        // batch itself
        txs.iter()
            .flat_map(|tx| tx.get_unauthenticated_notes().map(|note| note.id()))
            .filter(|note_id| !all_output_notes.remove(note_id))
            .collect()
    }
}

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

        // make sure that all unauthenticated notes in the transactions of the proposed batch
        // have been either created in any of the ready batches (or the batch itself) or are
        // already in the store
        //
        // TODO: this can be optimized by first computing dangling notes of the batch itself,
        //       and only then checking against the other ready batches
        let dangling_notes = self.find_dangling_notes(&txs).await;
        let found_unauthenticated_notes = match dangling_notes.is_empty() {
            true => Default::default(),
            false => {
                let stored_notes =
                    match self.store.get_note_authentication_info(dangling_notes.iter()).await {
                        Ok(stored_notes) => stored_notes,
                        Err(err) => return Err(BuildBatchError::NotePathsError(err, txs)),
                    };
                let missing_notes: Vec<_> = dangling_notes
                    .into_iter()
                    .filter(|note_id| !stored_notes.contains_note(note_id))
                    .collect();

                if !missing_notes.is_empty() {
                    return Err(BuildBatchError::UnauthenticatedNotesNotFound(missing_notes, txs));
                }

                stored_notes
            },
        };

        let batch = TransactionBatch::new(txs, found_unauthenticated_notes)?;

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

pub struct BatchProducer {
    pub batch_interval: Duration,
    pub workers: NonZeroUsize,
    pub mempool: Arc<Mutex<Mempool>>,
    pub tx_per_batch: usize,
    /// Used to simulate batch proving by sleeping for a random duration selected from this range.
    pub simulated_proof_time: Range<Duration>,
    /// Simulated block failure rate as a percentage.
    ///
    /// Note: this _must_ be sign positive and less than 1.0.
    pub failure_rate: f32,
}

type BatchResult = Result<(BatchJobId, TransactionBatch), (BatchJobId, BuildBatchError)>;

/// Wrapper around tokio's JoinSet that remains pending if the set is empty,
/// instead of returning None.
struct WorkerPool {
    in_progress: JoinSet<BatchResult>,
    simulated_proof_time: Range<Duration>,
    failure_rate: f32,
}

impl WorkerPool {
    fn new(simulated_proof_time: Range<Duration>, failure_rate: f32) -> Self {
        Self {
            simulated_proof_time,
            failure_rate,
            in_progress: JoinSet::new(),
        }
    }

    async fn join_next(&mut self) -> Result<BatchResult, tokio::task::JoinError> {
        if self.in_progress.is_empty() {
            std::future::pending().await
        } else {
            // Cannot be None as its not empty.
            self.in_progress.join_next().await.unwrap()
        }
    }

    fn len(&self) -> usize {
        self.in_progress.len()
    }

    fn spawn(&mut self, id: BatchJobId, transactions: Vec<AuthenticatedTransaction>) {
        self.in_progress.spawn({
            // Select a random work duration from the given proof range.
            let simulated_proof_time =
                rand::thread_rng().gen_range(self.simulated_proof_time.clone());

            // Randomly fail batches at the configured rate.
            //
            // Note: Rng::gen rolls between [0, 1.0) for f32, so this works as expected.
            let failed = rand::thread_rng().gen::<f32>() < self.failure_rate;

            async move {
                tracing::debug!("Begin proving batch.");

                // TODO: This is a deep clone which can be avoided by change batch building to using
                // refs or arcs.
                let transactions = transactions
                    .iter()
                    .map(AuthenticatedTransaction::raw_proven_transaction)
                    .cloned()
                    .collect();

                tokio::time::sleep(simulated_proof_time).await;
                if failed {
                    tracing::debug!("Batch proof failure injected.");
                    return Err((id, BuildBatchError::InjectedFailure(transactions)));
                }

                let batch = TransactionBatch::new(transactions, Default::default())
                    .map_err(|err| (id, err))?;

                tracing::debug!("Batch proof completed.");

                Ok((id, batch))
            }
        });
    }
}

impl BatchProducer {
    /// Starts the [BatchProducer], creating and proving batches at the configured interval.
    ///
    /// A pool of batch-proving workers is spawned, which are fed new batch jobs periodically.
    /// A batch is skipped if there are no available workers, or if there are no transactions
    /// available to batch.
    pub async fn run(self) {
        assert!(
            self.failure_rate < 1.0 && self.failure_rate.is_sign_positive(),
            "Failure rate must be a percentage"
        );

        let mut interval = tokio::time::interval(self.batch_interval);
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        let mut inflight = WorkerPool::new(self.simulated_proof_time, self.failure_rate);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if inflight.len() >= self.workers.get() {
                        tracing::info!("All batch workers occupied.");
                        continue;
                    }

                    // Transactions available?
                    let Some((batch_id, transactions)) =
                        self.mempool.lock().await.select_batch()
                    else {
                        tracing::info!("No transactions available for batch.");
                        continue;
                    };

                    inflight.spawn(batch_id, transactions);
                },
                result = inflight.join_next() => {
                    let mut mempool = self.mempool.lock().await;
                    match result {
                        Err(err) => {
                            tracing::warn!(%err, "Batch job panic'd.")
                            // TODO: somehow embed the batch ID into the join error, though this doesn't seem possible?
                            // mempool.batch_failed(batch_id);
                        },
                        Ok(Err((batch_id, err))) => {
                            tracing::warn!(%batch_id, %err, "Batch job failed.");
                            mempool.batch_failed(batch_id);
                        },
                        Ok(Ok((batch_id, batch))) => {
                            mempool.batch_proved(batch_id, batch);
                        }
                    }
                }
            }
        }
    }
}
