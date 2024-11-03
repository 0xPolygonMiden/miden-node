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
    domain::transaction::AuthenticatedTransaction,
    mempool::{BatchJobId, Mempool},
    store::DefaultStore,
    ProvenTransaction, SharedRwVec, COMPONENT, SERVER_BUILD_BATCH_FREQUENCY,
};

// FIXME: fix the batch builder tests.
// #[cfg(test)]
// mod tests;

pub mod batch;
pub use batch::TransactionBatch;
use miden_node_utils::formatting::{format_array, format_blake3_digest};

use crate::{errors::BuildBatchError, store::Store};

// BATCH PROVER
// ================================================================================================

pub struct BatchProver {
    pub batch_interval: Duration,
    pub workers: NonZeroUsize,
    /// Used to simulate batch proving by sleeping for a random duration selected from this range.
    pub simulated_proof_time: Range<Duration>,
    /// Simulated block failure rate as a percentage.
    ///
    /// Note: this _must_ be sign positive and less than 1.0.
    pub failure_rate: f32,
}

impl Default for BatchProver {
    fn default() -> Self {
        Self {
            batch_interval: SERVER_BUILD_BATCH_FREQUENCY,
            // SAFETY: 2 is non-zero so this always succeeds.
            workers: 2.try_into().unwrap(),
            simulated_proof_time: Duration::ZERO..Duration::ZERO,
            failure_rate: 0.0,
        }
    }
}

impl BatchProver {
    /// Starts the [BatchProducer], creating and proving batches at the configured interval.
    ///
    /// A pool of batch-proving workers is spawned, which are fed new batch jobs periodically.
    /// A batch is skipped if there are no available workers, or if there are no transactions
    /// available to batch.
    pub async fn run(self, mempool: Arc<Mutex<Mempool>>) {
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
                        mempool.lock().await.select_batch()
                    else {
                        tracing::info!("No transactions available for batch.");
                        continue;
                    };

                    inflight.spawn(batch_id, transactions);
                },
                result = inflight.join_next() => {
                    let mut mempool = mempool.lock().await;
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

// BATCH WORKER
// ================================================================================================

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

    #[instrument(target = "miden-block-producer", skip_all, err, fields(batch_id))]
    async fn build_batch(&self, txs: Vec<ProvenTransaction>) -> Result<(), BuildBatchError> {
        let num_txs = txs.len();

        info!(target: COMPONENT, num_txs, "Building a transaction batch");
        debug!(target: COMPONENT, txs = %format_array(txs.iter().map(|tx| tx.id().to_hex())));

        // TODO: Found unauthenticated notes are no longer required.. potentially?
        let batch = TransactionBatch::new(txs, Default::default())?;

        info!(target: COMPONENT, "Transaction batch built");
        Span::current().record("batch_id", format_blake3_digest(batch.id()));

        Ok(())
    }
}
