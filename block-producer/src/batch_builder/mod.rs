use std::{fmt::Debug, sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::{sync::RwLock, time};

use crate::block_builder::BlockBuilder;

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

pub struct BatchBuilderOptions {
    /// The frequency at which blocks are created
    pub block_frequency: Duration,
}

pub struct DefaultBatchBuilder<BB>
where
    BB: BlockBuilder,
{
    /// Batches ready to be included in a block
    ready_batches: Arc<RwLock<Vec<Arc<TransactionBatch>>>>,

    block_builder: Arc<BB>,

    options: BatchBuilderOptions,
}

impl<BB> DefaultBatchBuilder<BB>
where
    BB: BlockBuilder,
{
    pub fn new(
        block_builder: Arc<BB>,
        options: BatchBuilderOptions,
    ) -> Self {
        Self {
            ready_batches: Arc::new(RwLock::new(Vec::new())),
            block_builder,
            options,
        }
    }

    pub async fn run(self) {
        let mut interval = time::interval(self.options.block_frequency);

        loop {
            interval.tick().await;
            self.try_send_batches().await;
        }
    }

    async fn try_send_batches(&self) {
        let mut locked_ready_batches = self.ready_batches.write().await;

        if locked_ready_batches.is_empty() {
            return;
        }

        match self.block_builder.add_batches(locked_ready_batches.clone()) {
            Ok(_) => {
                // transaction groups were successfully sent, so drain the queue
                locked_ready_batches.truncate(0);
            },
            Err(_) => {
                // Batches were not sent, and remain in the queue. Do nothing.
            },
        }
    }
}

#[async_trait]
impl<BB> BatchBuilder for DefaultBatchBuilder<BB>
where
    BB: BlockBuilder,
{
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
async fn groups_to_batches(
    tx_groups: Vec<Vec<Arc<ProvenTransaction>>>
) -> Vec<Arc<TransactionBatch>> {
    // Note: in the future, this will send jobs to a cluster to transform groups into batches
    tx_groups.into_iter().map(|txs| Arc::new(TransactionBatch::new(txs))).collect()
}
