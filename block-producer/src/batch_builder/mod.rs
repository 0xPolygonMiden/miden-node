use std::{fmt::Debug, sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::{sync::RwLock, time};

use crate::txqueue::TransactionQueue;

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

pub struct BatchBuilderOptions {
    /// The frequency at which we fetch transactions from the transaction queue
    pub get_transactions_frequency: Duration,

    /// The size of a batch
    pub batch_size: usize,
}

pub struct DefaultBatchBuilder<TQ>
where
    TQ: TransactionQueue,
{
    batches: Arc<RwLock<Vec<TransactionBatch>>>,
    tx_queue: Arc<TQ>,
    options: BatchBuilderOptions,
}

impl<TQ> DefaultBatchBuilder<TQ>
where
    TQ: TransactionQueue,
{
    pub fn new(
        tx_queue: Arc<TQ>,
        options: BatchBuilderOptions,
    ) -> Self {
        Self {
            batches: Arc::new(RwLock::new(Vec::new())),
            tx_queue,
            options,
        }
    }

    pub async fn run(&self) {
        let mut interval = time::interval(self.options.get_transactions_frequency);

        loop {
            interval.tick().await;
        }
    }
}

#[async_trait]
impl<TQ> BatchBuilder for DefaultBatchBuilder<TQ>
where
    TQ: TransactionQueue,
{
    type AddBatchesError = ();

    async fn add_batches(
        &self,
        batches: Vec<TransactionBatch>,
    ) -> Result<(), Self::AddBatchesError> {
        todo!()
    }
}
