use std::{fmt::Debug, sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::{accounts::AccountId, notes::Nullifier, Digest};
use tokio::{sync::RwLock, time};

use crate::{batch_builder::BatchBuilder, store::TxInputsError, SharedProvenTx, SharedRwVec};

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
    ConsumedNotesAlreadyConsumed(Vec<Nullifier>),

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
}

impl From<TxInputsError> for VerifyTxError {
    fn from(err: TxInputsError) -> Self {
        Self::StoreConnectionFailed(err)
    }
}

#[async_trait]
pub trait TransactionVerifier: Send + Sync + 'static {
    async fn verify_tx(
        &self,
        tx: SharedProvenTx,
    ) -> Result<(), VerifyTxError>;
}

#[derive(Debug)]
pub enum AddTransactionError {
    VerificationFailed(VerifyTxError),
}

// TRANSACTION QUEUE
// ================================================================================================

#[async_trait]
pub trait TransactionQueue: Send + Sync + 'static {
    async fn add_transaction(
        &self,
        tx: SharedProvenTx,
    ) -> Result<(), AddTransactionError>;
}

// DEFAULT TRANSACTION QUEUE
// ================================================================================================

pub struct DefaultTransactionQueueOptions {
    /// The frequency at which we try to build batches from transactions in the queue
    pub build_batch_frequency: Duration,

    /// The size of a batch
    pub batch_size: usize,
}

pub struct DefaultTransactionQueue<BB, TV> {
    ready_queue: SharedRwVec<SharedProvenTx>,
    tx_verifier: Arc<TV>,
    batch_builder: Arc<BB>,
    options: DefaultTransactionQueueOptions,
}

impl<BB, TV> DefaultTransactionQueue<BB, TV>
where
    TV: TransactionVerifier,
    BB: BatchBuilder,
{
    pub fn new(
        tx_verifier: Arc<TV>,
        batch_builder: Arc<BB>,
        options: DefaultTransactionQueueOptions,
    ) -> Self {
        Self {
            ready_queue: Arc::new(RwLock::new(Vec::new())),
            tx_verifier,
            batch_builder,
            options,
        }
    }

    pub async fn run(self: Arc<Self>) {
        let mut interval = time::interval(self.options.build_batch_frequency);

        loop {
            interval.tick().await;
            self.try_build_batches().await;
        }
    }

    /// Divides the queue in groups to be batched; those that failed are appended back on the queue
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

            tokio::spawn(async move {
                match batch_builder.build_batch(txs.clone()).await {
                    Ok(_) => {
                        // batch was successfully built, do nothing
                    },
                    Err(_) => {
                        // batch building failed, add txs back at the end of the queue
                        ready_queue.write().await.append(&mut txs);
                    },
                }
            });
        }
    }
}

#[async_trait]
impl<BB, TV> TransactionQueue for DefaultTransactionQueue<BB, TV>
where
    TV: TransactionVerifier,
    BB: BatchBuilder,
{
    async fn add_transaction(
        &self,
        tx: SharedProvenTx,
    ) -> Result<(), AddTransactionError> {
        self.tx_verifier
            .verify_tx(tx.clone())
            .await
            .map_err(AddTransactionError::VerificationFailed)?;

        self.ready_queue.write().await.push(tx);

        Ok(())
    }
}
