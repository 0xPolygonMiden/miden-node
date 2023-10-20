use std::sync::Arc;

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::sync::RwLock;

#[async_trait]
pub trait TransactionQueueTrait {
    type AddTransactionError;

    async fn add_transaction(
        &self,
        tx: ProvenTransaction,
    ) -> Result<(), Self::AddTransactionError>;

    async fn get_transactions(&self) -> Vec<ProvenTransaction>;
}

pub struct TransactionQueue {
    ready_queue: Arc<RwLock<Vec<ProvenTransaction>>>,
}
