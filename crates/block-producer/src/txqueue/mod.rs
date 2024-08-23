use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::MAX_NOTES_PER_BATCH;
use tokio::{sync::RwLock, time};
use tracing::{debug, info, info_span, instrument, Instrument};

use crate::{
    batch_builder::BatchBuilder,
    errors::{AddTransactionError, VerifyTxError},
    ProvenTransaction, SharedRwVec, COMPONENT,
};

#[cfg(test)]
mod tests;

// TRANSACTION VALIDATOR
// ================================================================================================

/// Implementations are responsible to track in-flight transactions and verify that new transactions
/// added to the queue are not conflicting.
///
/// See [crate::store::ApplyBlock], that trait's `apply_block` is called when a block is sealed, and
/// it can determine when transactions are no longer in-flight.
#[async_trait]
pub trait TransactionValidator: Send + Sync + 'static {
    /// Method to receive a `tx` for processing.
    ///
    /// This method should:
    /// - Verify the transaction is valid, against the current's rollup state, and also against
    ///   in-flight transactions.
    /// - Track the necessary state of the transaction until it is committed to the `store`, to
    ///   perform the check above.
    async fn verify_tx(&self, tx: &ProvenTransaction) -> Result<Option<u32>, VerifyTxError>;
}

// TRANSACTION QUEUE
// ================================================================================================

pub struct TransactionQueueOptions {
    /// The frequency at which we try to build batches from transactions in the queue
    pub build_batch_frequency: Duration,

    /// The size of a batch
    pub batch_size: usize,
}

pub struct TransactionQueue<BB, TV> {
    ready_queue: SharedRwVec<ProvenTransaction>,
    tx_validator: Arc<TV>,
    batch_builder: Arc<BB>,
    options: TransactionQueueOptions,
}

impl<BB, TV> TransactionQueue<BB, TV>
where
    TV: TransactionValidator,
    BB: BatchBuilder,
{
    pub fn new(
        tx_validator: Arc<TV>,
        batch_builder: Arc<BB>,
        options: TransactionQueueOptions,
    ) -> Self {
        Self {
            ready_queue: Arc::new(RwLock::new(Vec::new())),
            tx_validator,
            batch_builder,
            options,
        }
    }

    pub async fn run(self: Arc<Self>) {
        let mut interval = time::interval(self.options.build_batch_frequency);

        info!(target: COMPONENT, period_ms = interval.period().as_millis(), "Transaction queue started");

        loop {
            interval.tick().await;
            self.try_build_batches().await;
        }
    }

    /// Divides the queue in groups to be batched; those that failed are appended back on the queue
    #[instrument(target = "miden-block-producer", skip_all)]
    async fn try_build_batches(&self) {
        let mut txs: Vec<ProvenTransaction> = {
            let mut locked_ready_queue = self.ready_queue.write().await;

            // If there are no transactions in the queue, this call is a no-op. The [BatchBuilder]
            // will produce empty blocks if necessary.
            if locked_ready_queue.is_empty() {
                debug!(target: COMPONENT, "Transaction queue empty");
                return;
            }

            locked_ready_queue.drain(..).rev().collect()
        };

        while !txs.is_empty() {
            let mut batch = Vec::with_capacity(self.options.batch_size);
            let mut notes_in_batch = 0;

            while let Some(tx) = txs.pop() {
                notes_in_batch += tx.output_notes().num_notes();

                debug_assert!(
                    tx.output_notes().num_notes() <= MAX_NOTES_PER_BATCH,
                    "Sanity check, the number of output notes of a single transaction must never be larger than the batch maximum",
                );

                if notes_in_batch > MAX_NOTES_PER_BATCH || batch.len() == self.options.batch_size {
                    // Batch would be too big in number of notes or transactions. Push the tx back
                    // to the list of available transactions and forward the current batch.
                    txs.push(tx);
                    break;
                }

                // The tx fits in the current batch
                batch.push(tx)
            }

            let ready_queue = self.ready_queue.clone();
            let batch_builder = self.batch_builder.clone();

            tokio::spawn(
                async move {
                    match batch_builder.build_batch(batch).await {
                        Ok(_) => {
                            // batch was successfully built, do nothing
                        },
                        Err(e) => {
                            // batch building failed, add txs back to the beginning of the queue
                            let mut locked_ready_queue = ready_queue.write().await;
                            e.into_transactions()
                                .into_iter()
                                .enumerate()
                                .for_each(|(i, tx)| locked_ready_queue.insert(i, tx));
                        },
                    }
                }
                .instrument(info_span!(target: COMPONENT, "batch_builder")),
            );
        }
    }

    /// Queues `tx` to be added in a batch and subsequently into a block.
    ///
    /// This method will validate the `tx` and ensure it is valid w.r.t. the rollup state, and the
    /// current in-flight transactions.
    #[instrument(target = "miden-block-producer", skip_all, err)]
    pub async fn add_transaction(&self, tx: ProvenTransaction) -> Result<Option<u32>, AddTransactionError> {
        info!(target: COMPONENT, tx_id = %tx.id().to_hex(), account_id = %tx.account_id().to_hex());

        let block_height = self.tx_validator
            .verify_tx(&tx)
            .await
            .map_err(AddTransactionError::VerificationFailed)?;

        let queue_len = {
            let mut queue_write_guard = self.ready_queue.write().await;
            queue_write_guard.push(tx);
            queue_write_guard.len()
        };

        info!(target: COMPONENT, queue_len, "Transaction added to tx queue");

        Ok(block_height)
    }
}
