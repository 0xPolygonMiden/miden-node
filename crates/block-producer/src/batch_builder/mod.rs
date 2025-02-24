use std::{num::NonZeroUsize, time::Duration};

use futures::{never::Never, FutureExt, TryFutureExt};
use miden_node_proto::domain::batch::BatchInputs;
use miden_node_utils::tracing::OpenTelemetrySpanExt;
use miden_objects::{
    batch::{BatchId, ProposedBatch, ProvenBatch},
    MIN_PROOF_SECURITY_LEVEL,
};
use miden_proving_service_client::proving_service::batch_prover::RemoteBatchProver;
use miden_tx_batch_prover::LocalBatchProver;
use rand::Rng;
use tokio::{task::JoinSet, time};
use tracing::{info, instrument, Instrument, Span};
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
    worker_pool: JoinSet<()>,
    batch_interval: Duration,
    /// The batch prover to use.
    ///
    /// If not provided, a local batch prover is used.
    batch_prover: BatchProver,
    /// Simulated block failure rate as a percentage.
    ///
    /// Note: this _must_ be sign positive and less than 1.0.
    failure_rate: f64,
    store: StoreClient,
}

impl BatchBuilder {
    /// Creates a new [`BatchBuilder`] with the given batch prover URL.
    ///
    /// If no URL is provided, a local batch prover is used.
    pub fn new(store: StoreClient, workers: NonZeroUsize, batch_prover_url: Option<Url>) -> Self {
        let batch_prover = match batch_prover_url {
            Some(url) => BatchProver::new_remote(url),
            None => BatchProver::new_local(MIN_PROOF_SECURITY_LEVEL),
        };

        let worker_pool = std::iter::repeat_n(std::future::ready(()), workers.get()).collect();

        Self {
            batch_interval: SERVER_BUILD_BATCH_FREQUENCY,
            worker_pool,
            failure_rate: 0.0,
            batch_prover,
            store,
        }
    }

    /// Starts the [`BatchBuilder`], creating and proving batches at the configured interval.
    ///
    /// A pool of batch-proving workers is spawned, which are fed new batch jobs periodically.
    /// A batch is skipped if there are no available workers, or if there are no transactions
    /// available to batch.
    pub async fn run(mut self, mempool: SharedMempool) {
        assert!(
            self.failure_rate < 1.0 && self.failure_rate.is_sign_positive(),
            "Failure rate must be a percentage"
        );

        let mut interval = tokio::time::interval(self.batch_interval);
        // We set the inverval's missed tick behaviour to burst. This means we'll catch up missed
        // batches as fast as possible. In other words, we try our best to keep the desired batch
        // interval on average. The other options would result in at least one skipped batch.
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Burst);

        loop {
            interval.tick().await;
            self.build_batch(mempool.clone()).await;
        }
    }

    #[instrument(parent = None, target = COMPONENT, name = "batch_builder.build_batch", skip_all)]
    async fn build_batch(&mut self, mempool: SharedMempool) {
        self.wait_for_available_worker().await;

        let job = BatchJob {
            failure_rate: self.failure_rate.clone(),
            store: self.store.clone(),
            mempool,
            batch_prover: self.batch_prover.clone(),
        };

        let root_span = Span::current();

        self.worker_pool
            .spawn(async move { job.build_batch().await }.instrument(root_span));
    }

    async fn wait_for_available_worker(&mut self) {
        if let Err(crash) = self.worker_pool.join_next().await.expect("worker pool is never empty")
        {
            tracing::error!(message=%crash, "Batch worker panic'd");
            panic!("Batch monitor panic'd: {crash}");
        }
    }
}

struct BatchJob {
    /// Simulated block failure rate as a percentage.
    ///
    /// Note: this _must_ be sign positive and less than 1.0.
    failure_rate: f64,
    store: StoreClient,
    batch_prover: BatchProver,
    mempool: SharedMempool,
}

impl BatchJob {
    async fn build_batch(&self) {
        let Some(batch) = self.select_batch().await else {
            tracing::info!("No transactions available.");
            return;
        };

        let batch_id = batch.id;

        self.get_batch_inputs(batch)
            .and_then(|(txs, inputs)| async { Self::propose_batch(txs, inputs) })
            .and_then(|proposed| self.prove_batch(proposed))
            // Failure must be injected before the final pipeline stage i.e. before commit is called. The system cannot
            // handle errors after it considers the process complete (which makes sense).
            .and_then(|x| self.inject_failure(x))
            .and_then(|proven_batch| async { Ok(self.commit_batch(proven_batch).await) })
            .or_else(|_err| self.rollback_batch(batch_id).never_error())
            // Error has been handled, this is just type manipulation to remove the result wrapper.
            .unwrap_or_else(|_: Never| ())
            .await;
    }

    async fn select_batch(&self) -> Option<SelectedBatch> {
        self.mempool
            .lock()
            .await
            .select_batch()
            .map(|(id, transactions)| SelectedBatch { id, transactions })
    }

    async fn get_batch_inputs(
        &self,
        batch: SelectedBatch,
    ) -> Result<(Vec<AuthenticatedTransaction>, BatchInputs), BuildBatchError> {
        let block_references =
            batch.transactions.iter().map(AuthenticatedTransaction::reference_block);
        let unauthenticated_notes = batch
            .transactions
            .iter()
            .flat_map(AuthenticatedTransaction::unauthenticated_notes);

        self.store
            .get_batch_inputs(block_references, unauthenticated_notes)
            .await
            .map_err(BuildBatchError::FetchBatchInputsFailed)
            .map(|inputs| (batch.transactions, inputs))
    }

    fn propose_batch(
        transactions: Vec<AuthenticatedTransaction>,
        inputs: BatchInputs,
    ) -> Result<ProposedBatch, BuildBatchError> {
        let transactions =
            transactions.iter().map(AuthenticatedTransaction::proven_transaction).collect();

        ProposedBatch::new(
            transactions,
            inputs.batch_reference_block_header,
            inputs.chain_mmr,
            inputs.note_proofs,
        )
        .map_err(BuildBatchError::ProposeBatchError)
    }

    async fn prove_batch(
        &self,
        proposed_batch: ProposedBatch,
    ) -> Result<ProvenBatch, BuildBatchError> {
        self.batch_prover.prove(proposed_batch).await
    }

    async fn inject_failure<T>(&self, value: T) -> Result<T, BuildBatchError> {
        let roll = rand::thread_rng().r#gen::<f64>();

        Span::current().set_attribute("failure_rate", self.failure_rate);
        Span::current().set_attribute("dice_roll", roll);

        if roll < self.failure_rate {
            Err(BuildBatchError::InjectedFailure)
        } else {
            Ok(value)
        }
    }

    async fn commit_batch(&self, batch: ProvenBatch) {
        self.mempool.lock().await.batch_proved(batch);
    }

    async fn rollback_batch(&self, batch_id: BatchId) {
        self.mempool.lock().await.batch_failed(batch_id);
    }
}

struct SelectedBatch {
    id: BatchId,
    transactions: Vec<AuthenticatedTransaction>,
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
            Self::Remote(prover) => {
                prover.prove(proposed_batch).await.map_err(BuildBatchError::RemoteProverError)
            },
            Self::Local(prover) => tokio::task::spawn_blocking({
                let prover = prover.clone();
                move || prover.prove(proposed_batch).map_err(BuildBatchError::ProveBatchError)
            })
            .await
            .map_err(BuildBatchError::JoinError)?,
        }
    }
}
