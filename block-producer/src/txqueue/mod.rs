use std::sync::Arc;

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::sync::RwLock;

use crate::state_view::TransactionVerifier;

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

pub struct DefaultTransactionQueue<StateView: TransactionVerifier> {
    ready_queue: Arc<RwLock<Vec<Arc<ProvenTransaction>>>>,
    state_view: Arc<StateView>,
}

impl<StateView> DefaultTransactionQueue<StateView>
where
    StateView: TransactionVerifier,
{
    pub fn new(state_view: Arc<StateView>) -> Self {
        Self {
            ready_queue: Arc::new(RwLock::new(Vec::new())),
            state_view,
        }
    }
}

#[async_trait]
impl<StateView> TransactionQueue for DefaultTransactionQueue<StateView>
where
    StateView: TransactionVerifier,
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
