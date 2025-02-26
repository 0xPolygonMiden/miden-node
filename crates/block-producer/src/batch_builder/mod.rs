use std::{num::NonZeroUsize, ops::Range, time::Duration};

use miden_node_proto::domain::batch::BatchInputs;
use miden_node_utils::formatting::format_array;
use miden_objects::{
    batch::{BatchId, ProposedBatch, ProvenBatch},
    MIN_PROOF_SECURITY_LEVEL,
};
use miden_proving_service_client::proving_service::batch_prover::RemoteBatchProver;
use miden_tx_batch_prover::LocalBatchProver;
use rand::Rng;
use tokio::{task::JoinSet, time};
use tracing::{debug, info, instrument, Span};
use url::Url;

use crate::{
    domain::transaction::AuthenticatedTransaction, errors::BuildBatchError, mempool::SharedMempool,
    store::StoreClient, COMPONENT, SERVER_BUILD_BATCH_FREQUENCY,
};

// BATCH BUILDER
// ================================================================================================

/// Builds [`TransactionBatch`] from sets of transactions.
///
/// Transaction sets are pulled from the [Mempool] at a configurable interval, and passed to a pool
/// of provers for proof generation. Proving is currently unimplemented and is instead simulated via
/// the given proof time and failure rate.
pub struct BatchBuilder {
    pub batch_interval: Duration,
    pub workers: NonZeroUsize,
    /// Used to simulate batch proving by sleeping for a random duration selected from this range.
    pub simulated_proof_time: Range<Duration>,
    /// Simulated block failure rate as a percentage.
    ///
    /// Note: this _must_ be sign positive and less than 1.0.
    pub failure_rate: f32,
    /// The batch prover to use.
    batch_prover: BatchProver,
}

impl BatchBuilder {
    /// Creates a new [`BatchBuilder`] with the given batch prover URL.
    ///
    /// Defaults to [`BatchProver::Local`] is no URL is provided.
    pub fn new(batch_prover_url: Option<Url>) -> Self {
        let batch_prover = match batch_prover_url {
            Some(url) => BatchProver::new_remote(url),
            None => BatchProver::new_local(MIN_PROOF_SECURITY_LEVEL),
        };

        Self {
            batch_interval: SERVER_BUILD_BATCH_FREQUENCY,
            // SAFETY: 2 is non-zero so this always succeeds.
            workers: 2.try_into().unwrap(),
            // Note: The range cannot be empty.
            simulated_proof_time: Duration::ZERO..Duration::from_millis(1),
            failure_rate: 0.0,
            batch_prover,
        }
    }

    /// Starts the [`BatchBuilder`], creating and proving batches at the configured interval.
    ///
    /// A pool of batch-proving workers is spawned, which are fed new batch jobs periodically.
    /// A batch is skipped if there are no available workers, or if there are no transactions
    /// available to batch.
    pub async fn run(self, mempool: SharedMempool, store: StoreClient) {
        assert!(
            self.failure_rate < 1.0 && self.failure_rate.is_sign_positive(),
            "Failure rate must be a percentage"
        );

        let mut interval = tokio::time::interval(self.batch_interval);
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        let mut worker_pool = WorkerPool::new(
            self.workers,
            self.simulated_proof_time,
            self.failure_rate,
            store,
            self.batch_prover,
        );

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if !worker_pool.has_capacity() {
                        tracing::info!("All batch workers occupied.");
                        continue;
                    }

                    // Transactions available?
                    let Some((batch_id, transactions)) =
                        mempool.lock().await.select_batch()
                    else {
                        tracing::info!("No transactions available for batch.");
                        continue;
                    };

                    worker_pool.spawn(batch_id, transactions).expect("Worker capacity was checked");
                },
                result = worker_pool.join_next() => {
                    let mut mempool = mempool.lock().await;
                    match result {
                        Err((batch_id, err)) => {
                            tracing::warn!(%batch_id, %err, "Batch job failed.");
                            mempool.batch_failed(batch_id);
                        },
                        Ok(batch) => {
                            mempool.batch_proved(batch);
                        }
                    }
                }
            }
        }
    }
}

// BATCH WORKER
// ================================================================================================

type BatchResult = Result<ProvenBatch, (BatchId, BuildBatchError)>;

/// Represents a pool of batch provers.
///
/// Effectively a wrapper around tokio's `JoinSet` that remains pending if the set is empty,
/// instead of returning None.
struct WorkerPool {
    in_progress: JoinSet<BatchResult>,
    simulated_proof_time: Range<Duration>,
    failure_rate: f32,
    /// Maximum number of workers allowed.
    capacity: NonZeroUsize,
    /// Maps spawned tasks to their job ID.
    ///
    /// This allows us to map panic'd tasks to the job ID. Uses [Vec] because the task ID does not
    /// implement [Ord]. Given that the expected capacity is relatively low, this has no real
    /// impact beyond ergonomics.
    task_map: Vec<(tokio::task::Id, BatchId)>,
    store: StoreClient,
    batch_prover: BatchProver,
}

impl WorkerPool {
    fn new(
        capacity: NonZeroUsize,
        simulated_proof_time: Range<Duration>,
        failure_rate: f32,
        store: StoreClient,
        batch_prover: BatchProver,
    ) -> Self {
        Self {
            simulated_proof_time,
            failure_rate,
            capacity,
            store,
            in_progress: JoinSet::default(),
            task_map: Vec::default(),
            batch_prover,
        }
    }

    /// Returns the next batch proof result.
    ///
    /// Will return pending if there are no jobs in progress (unlike tokio's [`JoinSet::join_next`]
    /// which returns an option).
    async fn join_next(&mut self) -> BatchResult {
        if self.in_progress.is_empty() {
            return std::future::pending().await;
        }

        let result = self
            .in_progress
            .join_next()
            .await
            .expect("JoinSet::join_next must be Some as the set is not empty")
            .map_err(|join_err| {
                // Map task ID to job ID as otherwise the caller can't tell which batch failed.
                //
                // Note that the mapping cleanup happens lower down.
                let batch_id = self
                    .task_map
                    .iter()
                    .find(|(task_id, _)| &join_err.id() == task_id)
                    .expect("Task ID should be in the task map")
                    .1;

                (batch_id, join_err.into())
            })
            .and_then(|x| x);

        // Cleanup task mapping by removing the result's task. This is inefficient but does not
        // matter as the capacity is expected to be low.
        let job_id = match &result {
            Ok(batch) => batch.id(),
            Err((id, _)) => *id,
        };
        self.task_map.retain(|(_, elem_job_id)| *elem_job_id != job_id);

        result
    }

    /// Returns `true` if there is a worker available.
    fn has_capacity(&self) -> bool {
        self.in_progress.len() < self.capacity.get()
    }

    /// Spawns a new batch proving task on the worker pool.
    ///
    /// # Errors
    ///
    /// Returns an error if no workers are available which can be checked using
    /// [`has_capacity`](Self::has_capacity).
    fn spawn(
        &mut self,
        id: BatchId,
        transactions: Vec<AuthenticatedTransaction>,
    ) -> Result<(), ()> {
        if !self.has_capacity() {
            return Err(());
        }

        let task_id = self
            .in_progress
            .spawn({
                // Select a random work duration from the given proof range.
                let simulated_proof_time =
                    rand::thread_rng().gen_range(self.simulated_proof_time.clone());

                // Randomly fail batches at the configured rate.
                //
                // Note: Rng::gen rolls between [0, 1.0) for f32, so this works as expected.
                let failed = rand::thread_rng().gen::<f32>() < self.failure_rate;
                let store = self.store.clone();
                let batch_prover = self.batch_prover.clone();

                async move {
                    tracing::debug!("Begin proving batch.");

                    let block_references =
                        transactions.iter().map(AuthenticatedTransaction::reference_block);
                    let unauthenticated_notes = transactions
                        .iter()
                        .flat_map(AuthenticatedTransaction::unauthenticated_notes);

                    let batch_inputs = store
                        .get_batch_inputs(block_references, unauthenticated_notes)
                        .await
                        .map_err(|err| (id, BuildBatchError::FetchBatchInputsFailed(err)))?;

                    let batch = Self::build_batch(transactions, batch_inputs, batch_prover)
                        .await
                        .map_err(|err| (id, err))?;

                    tokio::time::sleep(simulated_proof_time).await;
                    if failed {
                        tracing::debug!("Batch proof failure injected.");
                        return Err((id, BuildBatchError::InjectedFailure));
                    }

                    tracing::debug!("Batch proof completed.");

                    Ok(batch)
                }
            })
            .id();

        self.task_map.push((task_id, id));

        Ok(())
    }

    #[instrument(target = COMPONENT, skip_all, err, fields(batch_id))]
    async fn build_batch(
        txs: Vec<AuthenticatedTransaction>,
        batch_inputs: BatchInputs,
        batch_prover: BatchProver,
    ) -> Result<ProvenBatch, BuildBatchError> {
        let num_txs = txs.len();

        info!(target: COMPONENT, num_txs, "Building a transaction batch");
        debug!(target: COMPONENT, txs = %format_array(txs.iter().map(|tx| tx.id().to_hex())));

        let BatchInputs {
            batch_reference_block_header,
            note_proofs,
            chain_mmr,
        } = batch_inputs;

        let transactions = txs.iter().map(AuthenticatedTransaction::proven_transaction).collect();

        let proposed_batch =
            ProposedBatch::new(transactions, batch_reference_block_header, chain_mmr, note_proofs)
                .map_err(BuildBatchError::ProposeBatchError)?;

        Span::current().record("batch_id", proposed_batch.id().to_string());
        info!(target: COMPONENT, "Proposed Batch built");

        let proven_batch = batch_prover.prove(proposed_batch).await?;

        Span::current().record("batch_id", proven_batch.id().to_string());
        info!(target: COMPONENT, "Proven Batch built");

        Ok(proven_batch)
    }
}

// BATCH PROVER
// ================================================================================================

/// Represents a batch prover which can be either local or remote.
#[derive(Clone)]
pub enum BatchProver {
    Local(LocalBatchProver),
    Remote(RemoteBatchProver),
}

impl BatchProver {
    pub fn new_local(security_level: u32) -> Self {
        info!(target: COMPONENT, "Using local batch prover");
        Self::Local(LocalBatchProver::new(security_level))
    }

    pub fn new_remote(endpoint: impl Into<String>) -> Self {
        info!(target: COMPONENT, "Using remote batch prover");
        Self::Remote(RemoteBatchProver::new(endpoint))
    }

    #[instrument(target = COMPONENT, skip_all, err)]
    pub async fn prove(
        &self,
        proposed_batch: ProposedBatch,
    ) -> Result<ProvenBatch, BuildBatchError> {
        match self {
            Self::Local(prover) => {
                prover.prove(proposed_batch).map_err(BuildBatchError::ProveBatchError)
            },
            Self::Remote(prover) => {
                prover.prove(proposed_batch).await.map_err(BuildBatchError::RemoteProverError)
            },
        }
    }
}
