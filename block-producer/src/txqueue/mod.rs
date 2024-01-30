use std::{
    fmt::{Debug, Display, Formatter},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use miden_objects::{
    accounts::AccountId, notes::Nullifier, transaction::InputNotes, Digest, TransactionInputError,
};
use tokio::{sync::RwLock, time};
use tracing::{info, info_span, instrument, Instrument};

use crate::{
    batch_builder::BatchBuilder, store::TxInputsError, SharedProvenTx, SharedRwVec, COMPONENT,
};

#[cfg(test)]
mod tests;

// TRANSACTION VERIFIER
// ================================================================================================

#[derive(Debug, PartialEq)]
pub enum VerifyTxError {
    /// The account that the transaction modifies has already been modified and isn't yet committed
    /// to a block
    AccountAlreadyModifiedByOtherTx(AccountId),

    /// Another transaction already consumed the notes with given nullifiers
    InputNotesAlreadyConsumed(InputNotes<Nullifier>),

    /// The account's initial hash did not match the current account's hash
    IncorrectAccountInitialHash {
        tx_initial_account_hash: Digest,
        store_account_hash: Option<Digest>,
    },

    /// Failed to retrieve transaction inputs from the store
    ///
    /// TODO: Make this an "internal error". Q: Should we have a single `InternalError` enum for all
    /// internal errors that can occur across the system?
    StoreConnectionFailed(TxInputsError),

    TransactionInputError(TransactionInputError),
}

impl Display for VerifyTxError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl From<TxInputsError> for VerifyTxError {
    fn from(err: TxInputsError) -> Self {
        Self::StoreConnectionFailed(err)
    }
}

impl From<TransactionInputError> for VerifyTxError {
    fn from(value: TransactionInputError) -> Self {
        Self::TransactionInputError(value)
    }
}

/// Implementations are responsible to track in-flight transactions and verify that new transactions
/// added to the queue are not conflicting.
///
/// See [crate::store::ApplyBlock], that trait's `apply_block` is called when a block is sealed, and
/// it can determine when transactions are no longer in-flight.
#[async_trait]
pub trait TransactionVerifier: Send + Sync + 'static {
    /// Method to receive a `tx` for processing.
    ///
    /// This method should:
    ///
    /// 1. Verify the transaction is valid, against the current's rollup state, and also against
    ///    in-flight transactions.
    /// 2. Track the necessary state of the transaction until it is commited to the `store`, to
    ///    perform the check above.
    async fn verify_tx(
        &self,
        tx: SharedProvenTx,
    ) -> Result<(), VerifyTxError>;
}

#[derive(Debug)]
pub enum AddTransactionError {
    VerificationFailed(VerifyTxError),
}

impl Display for AddTransactionError {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
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
    ready_queue: SharedRwVec<SharedProvenTx>,
    tx_verifier: Arc<TV>,
    batch_builder: Arc<BB>,
    options: TransactionQueueOptions,
}

impl<BB, TV> TransactionQueue<BB, TV>
where
    TV: TransactionVerifier,
    BB: BatchBuilder,
{
    pub fn new(
        tx_verifier: Arc<TV>,
        batch_builder: Arc<BB>,
        options: TransactionQueueOptions,
    ) -> Self {
        Self {
            ready_queue: Arc::new(RwLock::new(Vec::new())),
            tx_verifier,
            batch_builder,
            options,
        }
    }

    #[instrument(target = "miden-block-producer", name = "block_producer" skip_all)]
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
        let txs: Vec<SharedProvenTx> = {
            let mut locked_ready_queue = self.ready_queue.write().await;

            if locked_ready_queue.is_empty() {
                return;
            }

            locked_ready_queue.drain(..).collect()
        };

        let tx_groups = txs.chunks(self.options.batch_size).map(|txs| txs.to_vec());

        for mut txs in tx_groups {
            let ready_queue = self.ready_queue.clone();
            let batch_builder = self.batch_builder.clone();

            tokio::spawn(
                async move {
                    match batch_builder.build_batch(txs.clone()).await {
                        Ok(_) => {
                            // batch was successfully built, do nothing
                        },
                        Err(_) => {
                            // batch building failed, add txs back at the end of the queue
                            ready_queue.write().await.append(&mut txs);
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
    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(target = "miden-block-producer", skip_all, err)]
    pub async fn add_transaction(
        &self,
        tx: SharedProvenTx,
    ) -> Result<(), AddTransactionError> {
        info!(target: COMPONENT, tx_id = %tx.id().to_hex(), account_id = %tx.account_id().to_hex());

        self.tx_verifier
            .verify_tx(tx.clone())
            .await
            .map_err(AddTransactionError::VerificationFailed)?;

        let queue_len = {
            let mut queue_write_guard = self.ready_queue.write().await;
            queue_write_guard.push(tx);
            queue_write_guard.len()
        };

        info!(target: COMPONENT, queue_len, "Transaction added to tx queue");

        Ok(())
    }
}
