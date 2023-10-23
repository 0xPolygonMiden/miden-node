use std::{fmt::Debug, sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::{sync::RwLock, time};


pub struct TransactionBatch {
    txs: Vec<Arc<ProvenTransaction>>,
}

impl TransactionBatch {
    pub fn new(txs: Vec<Arc<ProvenTransaction>>) -> Self {
        Self { txs }
    }
}

#[async_trait]
pub trait BatchBuilder: Send + Sync + 'static {
    // TODO: Make concrete `AddBatches` Error?
    type AddBatchesError: Debug;

    async fn add_batches(
        &self,
        batches: Vec<TransactionBatch>,
    ) -> Result<(), Self::AddBatchesError>;
}

pub struct BatchBuilderOptions {}

pub struct DefaultBatchBuilder {
    batches: Arc<RwLock<Vec<TransactionBatch>>>,
    options: BatchBuilderOptions,
}

impl DefaultBatchBuilder {
    pub fn new(options: BatchBuilderOptions) -> Self {
        Self {
            batches: Arc::new(RwLock::new(Vec::new())),
            options,
        }
    }
}

#[async_trait]
impl BatchBuilder for DefaultBatchBuilder {
    type AddBatchesError = ();

    async fn add_batches(
        &self,
        mut txs: Vec<TransactionBatch>,
    ) -> Result<(), Self::AddBatchesError> {
        self.batches.write().await.append(&mut txs);

        Ok(())
    }
}
