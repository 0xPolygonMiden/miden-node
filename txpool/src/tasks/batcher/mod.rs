#[cfg(test)]
mod tests;

use async_trait::async_trait;
use core::fmt::Debug;
use miden_objects::transaction::ProvenTransaction;

use crate::TxBatch;

/// Encapsulates the means the communicate with the Batcher task.
#[async_trait]
pub trait BatcherTaskHandle {
    type SendError: Debug;
    type ReceiveError: Debug;

    /// Blocks until it is time to send a batch
    /// Usually a `tokio::sync::Notify` will be behind
    async fn wait_for_send_batch_notification(&self);

    /// Blocks until a new transaction is received
    /// Usually, either a channel or RPC endpoint will be behind
    async fn receive_tx(&self) -> Result<ProvenTransaction, Self::ReceiveError>;

    /// Send a batch.
    /// This MUST only be called after being notified with `notify_send_batch`.
    async fn send_batch(
        &self,
        txs: TxBatch,
    ) -> Result<(), Self::SendError>;
}

/// Starts the batcher task
pub async fn batcher_task<H: BatcherTaskHandle>(handle: H) {
    let mut task = BatcherTask::new(handle);
    task.run().await
}

struct BatcherTask<H: BatcherTaskHandle> {
    handle: H,
    txs: Vec<ProvenTransaction>,
}

impl<H: BatcherTaskHandle> BatcherTask<H> {
    pub fn new(handle: H) -> Self {
        Self {
            handle,
            txs: Vec::new(),
        }
    }

    pub async fn run(&mut self) {
        tokio::select! {
            _ = self.handle.wait_for_send_batch_notification() => {
                self.on_notify_send_batch().await
            }
            proven_tx = self.handle.receive_tx() => {
                let proven_tx = proven_tx.expect("Failed to receive tx");
                self.on_receive_tx(proven_tx).await
            }
        }
    }

    async fn on_notify_send_batch(&mut self) {
        println!("NOTIFICATION");
        let batch: TxBatch = self.txs.drain(..).collect();
        self.handle.send_batch(batch).await.expect("Failed to send batch");
    }

    async fn on_receive_tx(
        &mut self,
        proven_tx: ProvenTransaction,
    ) {
        self.txs.push(proven_tx);
    }
}
