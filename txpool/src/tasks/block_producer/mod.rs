use async_trait::async_trait;
use core::fmt::Debug;
use core::time::Duration;
use miden_objects::BlockHeader;
use tokio::time;

use crate::{Block, BlockData, TxBatch};

#[derive(Debug)]
pub enum Error {}

#[async_trait]
pub trait BlockProducerTaskHandle {
    type RecvError: Debug;
    type ApplyBlockError: Debug;

    /// Receive a new transaction batch
    async fn receive_tx_batch(&self) -> Result<TxBatch, Self::RecvError>;

    /// Output the produced block
    async fn apply_block(
        &self,
        block: Block,
    ) -> Result<(), Self::ApplyBlockError>;
}

pub struct BlockProducerTaskOptions {
    /// Desired rate at which to create blocks
    pub block_time: Duration,
}

pub async fn block_producer_task<H: BlockProducerTaskHandle>(
    handle: H,
    prev_header: BlockHeader,
    options: BlockProducerTaskOptions,
) {
    let mut task = BlockProducerTask::new(handle, prev_header, options);
    task.run().await
}

struct BlockProducerTask<H: BlockProducerTaskHandle> {
    handle: H,
    prev_header: BlockHeader,
    options: BlockProducerTaskOptions,
}

impl<H: BlockProducerTaskHandle> BlockProducerTask<H> {
    pub fn new(
        handle: H,
        prev_header: BlockHeader,
        options: BlockProducerTaskOptions,
    ) -> Self {
        Self {
            handle,
            prev_header,
            options,
        }
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

            let block = self.produce_block(tx_batch).await.expect("Error while producing block");

            self.handle.apply_block(block).await.expect("Error while applying block");
        }
    }

    async fn produce_block(
        &self,
        tx_batch: TxBatch,
    ) -> Result<Block, Error> {
        let updated_account_state_hashes: Vec<_> = tx_batch
            .into_iter()
            .map(|tx| (tx.account_id(), tx.final_account_hash()))
            .collect();

        let _block_body = BlockData {
            updated_account_state_hashes,
        };

        todo!()
    }
}
