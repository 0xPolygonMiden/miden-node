use async_trait::async_trait;
use core::fmt::Debug;
use core::time::Duration;
use miden_objects::{accounts::AccountId, crypto::merkle::PartialMerkleTree, BlockHeader, Digest};
use tokio::time;

use crate::{Block, BlockData, TxBatch};

#[derive(Debug)]
pub enum Error {}

#[async_trait]
pub trait BlockProducerTaskHandle {
    type RecvError: Debug;
    type MerkleTreesReqError: Debug;
    type ApplyBlockError: Debug;

    /// Receive a new transaction batch
    async fn receive_tx_batch(&self) -> Result<TxBatch, Self::RecvError>;

    /// Requests the partial merkle trees associated with the updated accounts, notes and nullifiers.
    /// TODO: also request for notes and nullifiers
    async fn request_partial_merkle_trees(
        &self,
        updated_account_ids: Vec<AccountId>,
    ) -> Result<PartialMerkleTree, Self::MerkleTreesReqError>;

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
        let parsed_tx_batch = ParsedTxBatch::from(tx_batch);

        let account_pmt = self
            .handle
            .request_partial_merkle_trees(parsed_tx_batch.updated_account_ids.clone())
            .await
            .expect("Request to get the partial merkle trees failed");

        // TODO: compute new account db root from pmt

        let _block_body = BlockData {
            updated_account_state_hashes: parsed_tx_batch.updated_account_state_hashes,
        };

        todo!()
    }
}

#[derive(Default)]
struct ParsedTxBatch {
    updated_account_ids: Vec<AccountId>,
    updated_account_state_hashes: Vec<(AccountId, Digest)>,
}

impl From<TxBatch> for ParsedTxBatch {
    fn from(tx_batch: TxBatch) -> Self {
        let mut parsed_tx_batch = ParsedTxBatch::default();

        for tx in tx_batch {
            parsed_tx_batch.updated_account_ids.push(tx.account_id());
            parsed_tx_batch
                .updated_account_state_hashes
                .push((tx.account_id(), tx.final_account_hash()))
        }

        parsed_tx_batch
    }
}
