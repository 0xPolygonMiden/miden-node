use async_trait::async_trait;
use core::fmt::Debug;
use core::time::Duration;
use tokio::time;

use crate::TxBatch;

#[async_trait]
pub trait BlockProducerTaskHandle {
    type RecvError: Debug;

    /// Receive a new transaction batch
    async fn receive_tx_batch(&self) -> Result<TxBatch, Self::RecvError>;
}

pub struct BlockProducerTaskOptions {
    /// Desired rate at which to create blocks
    pub block_time: Duration,
}

pub async fn block_producer_task<H: BlockProducerTaskHandle>(
    handle: H,
    options: BlockProducerTaskOptions,
) {
    let mut task = BlockProducerTask::new(handle, options);
    task.run().await
}

struct BlockProducerTask<H: BlockProducerTaskHandle> {
    handle: H,
    options: BlockProducerTaskOptions,
}

impl<H: BlockProducerTaskHandle> BlockProducerTask<H> {
    pub fn new(
        handle: H,
        options: BlockProducerTaskOptions,
    ) -> Self {
        Self { handle, options }
    }

    pub async fn run(&mut self) {
        let mut interval = time::interval(self.options.block_time);

        loop {
            interval.tick().await;

            let tx_batch = self
                .handle
                .receive_tx_batch()
                .await
                .expect("Failed to receive transaction batch");

            self.produce_block(tx_batch).await;
        }
    }

    async fn produce_block(
        &self,
        _tx_batch: TxBatch,
    ) {
        todo!()
    }
}
