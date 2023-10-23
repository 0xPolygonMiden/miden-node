use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::{sync::RwLock, time};

use crate::state_view::TransactionVerifier;

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
    pub send_batches_frequency: Duration,
}

pub struct DefaultTransactionQueue<TV: TransactionVerifier> {
    ready_queue: Arc<RwLock<Vec<Arc<ProvenTransaction>>>>,
    state_view: Arc<TV>,
    options: DefaultTransactionQueueOptions,
}

impl<TV> DefaultTransactionQueue<TV>
where
    TV: TransactionVerifier,
{
    pub fn new(
        state_view: Arc<TV>,
        options: DefaultTransactionQueueOptions,
    ) -> Self {
        Self {
            ready_queue: Arc::new(RwLock::new(Vec::new())),
            state_view,
            options,
        }
    }

    pub async fn run(self) {
        let mut interval = time::interval(self.options.send_batches_frequency);

        loop {
            interval.tick().await;
        }
    }
}

#[async_trait]
impl<TV> TransactionQueue for DefaultTransactionQueue<TV>
where
    TV: TransactionVerifier,
{
    type AddTransactionError = AddTransactionError;

    async fn add_transaction(
        &self,
        tx: Arc<ProvenTransaction>,
    ) -> Result<(), Self::AddTransactionError> {
        self.state_view
            .verify_tx(tx.clone())
            .await
            .map_err(|_| AddTransactionError::VerificationFailed)?;

        self.ready_queue.write().await.push(tx);

        Ok(())
    }
}
