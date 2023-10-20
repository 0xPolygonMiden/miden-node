use std::sync::Arc;

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::sync::RwLock;

use crate::state_view::StateViewTrait;

#[async_trait]
pub trait TransactionQueueTrait: Send + Sync + 'static {
    type AddTransactionError;

    async fn add_transaction(
        &self,
        tx: Arc<ProvenTransaction>,
    ) -> Result<(), Self::AddTransactionError>;

    async fn get_transactions(&self) -> Vec<ProvenTransaction>;
}

pub enum AddTransactionError {
    VerificationFailed,
}

pub struct TransactionQueue<StateView: StateViewTrait> {
    ready_queue: Arc<RwLock<Vec<Arc<ProvenTransaction>>>>,
    state_view: Arc<StateView>,
}

impl<StateView> TransactionQueue<StateView>
where
    StateView: StateViewTrait,
{
    pub fn new(state_view: Arc<StateView>) -> Self {
        Self {
            ready_queue: Arc::new(RwLock::new(Vec::new())),
            state_view,
        }
    }
}

#[async_trait]
impl<StateView> TransactionQueueTrait for TransactionQueue<StateView>
where
    StateView: StateViewTrait,
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

    async fn get_transactions(&self) -> Vec<ProvenTransaction> {
        todo!()
    }
}
