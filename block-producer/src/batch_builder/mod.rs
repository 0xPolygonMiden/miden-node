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

    async fn add_tx_groups(
        &self,
        tx_groups: Vec<Vec<Arc<ProvenTransaction>>>,
    ) -> Result<(), Self::AddBatchesError>;
}

pub struct BatchBuilderOptions {}

pub struct DefaultBatchBuilder {
    /// Batches ready to be included in a block
    ready_batches: Arc<RwLock<Vec<TransactionBatch>>>,

    options: BatchBuilderOptions,
}

impl DefaultBatchBuilder {
    pub fn new(options: BatchBuilderOptions) -> Self {
        Self {
            ready_batches: Arc::new(RwLock::new(Vec::new())),
            options,
        }
    }
}

#[async_trait]
impl BatchBuilder for DefaultBatchBuilder {
    type AddBatchesError = ();

    async fn add_tx_groups(
        &self,
        tx_groups: Vec<Vec<Arc<ProvenTransaction>>>,
    ) -> Result<(), Self::AddBatchesError> {
        let ready_batches = self.ready_batches.clone();

        tokio::spawn(async move {
            let mut batches = groups_to_batches(tx_groups).await;

            ready_batches.write().await.append(&mut batches);
        });

        Ok(())
    }
}

/// Transforms the transaction groups to transaction batches
async fn groups_to_batches(tx_groups: Vec<Vec<Arc<ProvenTransaction>>>) -> Vec<TransactionBatch> {
    // Note: in the future, this will send jobs to a cluster to transform groups into batches
    tx_groups.into_iter().map(|txs| TransactionBatch::new(txs)).collect()
}
