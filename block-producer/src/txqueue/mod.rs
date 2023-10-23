use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::{sync::RwLock, time};

use crate::{
    batch_builder::{BatchBuilder, TransactionBatch},
    state_view::TransactionVerifier,
};

// TRANSACTION QUEUE
// ================================================================================================

#[async_trait]
pub trait TransactionQueue: Send + Sync + 'static {
    type AddTransactionError;

    async fn add_transaction(
        &self,
        tx: Arc<ProvenTransaction>,
    ) -> Result<(), Self::AddTransactionError>;
}

pub enum AddTransactionError {
    VerificationFailed,
}

// DEFAULT TRANSACTION QUEUE
// ================================================================================================

pub struct DefaultTransactionQueueOptions {
    /// The frequency at which send try batches to
    pub send_batches_frequency: Duration,

    /// The size of a batch
    pub batch_size: usize,
}

pub struct DefaultTransactionQueue<BB: BatchBuilder, TV: TransactionVerifier> {
    ready_queue: Arc<RwLock<Vec<Arc<ProvenTransaction>>>>,
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
        let mut interval = time::interval(self.options.send_batches_frequency);

        loop {
            interval.tick().await;
            self.try_send_batches().await;
        }
    }

    async fn try_send_batches(&self) {
        let mut locked_ready_queue = self.ready_queue.write().await;

        if locked_ready_queue.is_empty() {
            return;
        }

        let batches: Vec<TransactionBatch> = locked_ready_queue
            .chunks(self.options.batch_size)
            .map(|txs| TransactionBatch::new(txs.to_vec()))
            .collect();

        match self.batch_builder.add_batches(batches).await {
            Ok(_) => {
                // batches we successfully sent, so drain the queue
                locked_ready_queue.truncate(0);
            },
            Err(_) => {
                // Batches were not sent, and remain in the queue. Do nothing.
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
    type AddTransactionError = AddTransactionError;

    async fn add_transaction(
        &self,
        tx: Arc<ProvenTransaction>,
    ) -> Result<(), Self::AddTransactionError> {
        self.tx_verifier
            .verify_tx(tx.clone())
            .await
            .map_err(|_| AddTransactionError::VerificationFailed)?;

        self.ready_queue.write().await.push(tx);

        Ok(())
    }
}
