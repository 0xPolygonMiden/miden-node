use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::{sync::RwLock, time};

use crate::txqueue::TransactionQueue;

pub struct TransactionBatch {
    batch: Vec<Arc<ProvenTransaction>>,
}

#[async_trait]
pub trait BatchBuilderTrait: Send + Sync + 'static {
    async fn get_batches(&self) -> Vec<TransactionBatch>;
}

pub struct BatchBuilderOptions {
    /// The frequency at which we fetch transactions from the transaction queue
    pub get_transactions_frequency: Duration,

    /// The size of a batch
    pub batch_size: usize,
}

pub struct BatchBuilder<TQ>
where
    TQ: TransactionQueue,
{
    batches: Arc<RwLock<Vec<TransactionBatch>>>,
    tx_queue: Arc<TQ>,
    options: BatchBuilderOptions,
}

impl<TQ> BatchBuilder<TQ>
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
            let txs = self.tx_queue.get_transactions().await;

            let mut batches: Vec<TransactionBatch> = txs
                .chunks(self.options.batch_size)
                .map(|txs| TransactionBatch {
                    batch: txs.to_vec(),
                })
                .collect();

            self.batches.write().await.append(&mut batches);

            interval.tick().await;
        }
    }
}

#[async_trait]
impl<TQ> BatchBuilderTrait for BatchBuilder<TQ>
where
    TQ: TransactionQueue,
{
    async fn get_batches(&self) -> Vec<TransactionBatch> {
        self.batches.write().await.drain(..).collect()
    }
}
