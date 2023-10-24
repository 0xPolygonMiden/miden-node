use std::{fmt::Debug, sync::Arc, time::Duration};

use async_trait::async_trait;
use tokio::{sync::RwLock, time};

use crate::{batch_builder::BatchBuilder, SharedProvenTx, SharedRwVec};

// TRANSACTION QUEUE
// ================================================================================================

#[derive(Debug)]
pub enum VerifyTxError {}

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
    /// The frequency at which we try to send transaction groups
    pub send_tx_groups_frequency: Duration,

    /// The size of a batch
    pub batch_size: usize,
}

pub struct DefaultTransactionQueue<BB: BatchBuilder, TV: TransactionVerifier> {
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

    pub async fn run(self) {
        let mut interval = time::interval(self.options.send_tx_groups_frequency);

        loop {
            interval.tick().await;
            self.try_send_tx_groups().await;
        }
    }

    async fn try_send_tx_groups(&self) {
        let mut locked_ready_queue = self.ready_queue.write().await;

        if locked_ready_queue.is_empty() {
            return;
        }

        let tx_groups: Vec<Vec<SharedProvenTx>> = locked_ready_queue
            .chunks(self.options.batch_size)
            .map(|txs| txs.to_vec())
            .collect();

        match self.batch_builder.build_batch(tx_groups).await {
            Ok(_) => {
                // Transaction groups were successfully sent, so drain the queue
                locked_ready_queue.truncate(0);
            },
            Err(_) => {
                // Transaction groups were not sent, and remain in the queue. Do nothing.
            },
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
            .map_err(|err| AddTransactionError::VerificationFailed(err))?;

        self.ready_queue.write().await.push(tx);

        Ok(())
    }
}
