use std::{marker::PhantomData, sync::Arc};

use async_trait::async_trait;
use concurrent_queue::ConcurrentQueue;
use miden_objects::transaction::ProvenTransaction;

#[async_trait]
pub trait TxQueueHandleIn {
    type ReadTxError;

    async fn read_transaction(&mut self) -> Result<ProvenTransaction, Self::ReadTxError>;
}

#[async_trait]
pub trait TxQueueHandleOut {
    type VerifyTxError;
    type ProduceBatchError;

    async fn verify_transaction(
        &self,
        tx: Arc<ProvenTransaction>,
    ) -> Result<(), Self::VerifyTxError>;

    async fn produce_batch(
        &self,
        txs: Vec<ProvenTransaction>,
    ) -> Result<(), Self::ProduceBatchError>;
}

pub struct TxQueueOptions {
    pub batch_size: usize,
}

pub async fn tx_queue<HandleIn, HandleOut>(
    handle_in: HandleIn,
    handle_out: HandleOut,
    options: TxQueueOptions,
) where
    HandleIn: TxQueueHandleIn,
    HandleOut: TxQueueHandleOut,
{
    let mut queue_task = TxQueue::new(handle_in, handle_out, options);
    queue_task.run().await
}

struct TxQueue<HandleIn, HandleOut>
where
    HandleIn: TxQueueHandleIn,
    HandleOut: TxQueueHandleOut,
{
    queue: ConcurrentQueue<Arc<ProvenTransaction>>,
    handle_in: HandleIn,
    handle_out: HandleOut,
    options: TxQueueOptions,
}

impl<HandleIn, HandleOut> TxQueue<HandleIn, HandleOut>
where
    HandleIn: TxQueueHandleIn,
    HandleOut: TxQueueHandleOut,
{
    pub fn new(
        handle_in: HandleIn,
        handle_out: HandleOut,
        options: TxQueueOptions,
    ) -> Self {
        Self {
            queue: ConcurrentQueue::unbounded(),
            handle_in,
            handle_out,
            options,
        }
    }

    pub async fn run(&mut self) {
        todo!()
    }
}
