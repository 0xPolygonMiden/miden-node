#![allow(unused)]

use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    fmt::Display,
    ops::Sub,
    sync::Arc,
};

use batch_graph::BatchGraph;
use miden_objects::{
    accounts::AccountId,
    transaction::{ProvenTransaction, TransactionId},
    Digest,
};
use miden_tx::utils::collections::KvMap;
use transaction_graph::TransactionGraph;

use crate::store::TransactionInputs;

mod batch_graph;
mod transaction_graph;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BatchId(u64);

impl Display for BatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl BatchId {
    pub fn increment(mut self) {
        self.0 += 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockNumber(u32);

impl Display for BlockNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl BlockNumber {
    pub fn increment(mut self) {
        self.0 += 1;
    }

    pub fn checked_sub(&self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }
}

pub struct Mempool {
    /// The latest inflight state of each account.
    ///
    /// Accounts without inflight transactions are not stored.
    account_state: BTreeMap<AccountId, (Digest, TransactionId)>,

    /// The number of the next block we hand out.
    next_inflight_block: BlockNumber,

    /// Inflight transactions.
    transactions: TransactionGraph,

    /// Inflight batches.
    batches: BatchGraph,

    /// The next batches ID.
    next_batch_id: BatchId,

    /// Blocks which are inflight or completed but not yet considered stale.
    block_pool: BTreeMap<BlockNumber, Vec<BatchId>>,

    /// The current block height of the chain.
    completed_blocks: BlockNumber,

    /// Number of blocks before transaction input is considered stale.
    staleness: BlockNumber,
}

impl Mempool {
    /// Complete barring todos.
    pub fn add_transaction(
        &mut self,
        transaction: ProvenTransaction,
        mut inputs: TransactionInputs,
    ) -> Result<u32, AddTransactionError> {
        // Ensure inputs aren't stale.
        if let Some(stale_block) = self.stale_block() {
            if inputs.current_block_height <= stale_block.0 {
                return Err(AddTransactionError::StaleInputs {
                    input_block: BlockNumber(inputs.current_block_height),
                    stale_limit: stale_block,
                });
            }
        }

        let account_update = transaction.account_update();

        // Inflight transactions upon which this new transaction depends due to building on their
        // outputs.
        let mut parents = BTreeSet::new();

        // Merge inflight state with inputs.
        //
        // This gives us the latest applicable state for this transaction.
        // TODO: notes and nullifiers.
        if let Some((state, parent)) = self.account_state.get(&account_update.account_id()) {
            parents.insert(*parent);
            inputs.account_hash = Some(*state);
        }

        // Verify transaction input state.
        // TODO: update notes and nullifiers.
        if inputs.account_hash.unwrap_or_default() != account_update.init_state_hash() {
            return Err(AddTransactionError::InvalidAccountState {
                current: inputs.account_hash.unwrap_or_default(),
                expected: account_update.init_state_hash(),
            });
        }

        // Transaction is valid, update inflight state.
        // TODO: update notes and nullifiers.
        let tx_id = transaction.id();
        self.account_state
            .insert(transaction.account_id(), (account_update.final_state_hash(), tx_id));

        self.transactions.insert(transaction, parents);

        Ok(self.completed_blocks.0)
    }

    /// Returns at most `count` transactions and a batch ID.
    ///
    /// Transactions are returned in a valid execution ordering.
    ///
    /// Returns `None` if no transactions are available.
    pub fn select_batch(&mut self, count: usize) -> Option<(BatchId, Vec<Arc<ProvenTransaction>>)> {
        let mut parents = BTreeSet::new();
        let mut batch = Vec::with_capacity(count);

        for _ in 0..count {
            // Select transactions according to some strategy here. For now its just arbitrary.
            let Some((tx, tx_parents)) = self.transactions.pop_for_batching() else {
                break;
            };
            batch.push(tx);
            parents.extend(tx_parents);
        }

        // Update the depedency graph to reflect parents at the batch level by removing
        // all edges within this batch.
        for tx in &batch {
            parents.remove(&tx.id());
        }

        let batch_id = self.next_batch_id;
        self.next_batch_id.increment();

        let txs = batch.iter().map(|tx| tx.id()).collect();
        self.batches.insert(batch_id, txs, parents);

        Some((batch_id, batch))
    }

    /// Drops the failed batch and all of its descendents.
    ///
    /// Transactions are placed back in the queue.
    pub fn batch_failed(&mut self, batch: BatchId) {
        let removed_batches = self.batches.purge_subgraph(batch);

        // Its possible to receive failures for batches which were already removed
        // as part of a prior failure. Early exit to prevent logging these no-ops.
        if removed_batches.is_empty() {
            return;
        }

        let batches = removed_batches.iter().map(|(b, _)| *b).collect::<Vec<_>>();
        let transactions = removed_batches.into_iter().flat_map(|(_, tx)| tx.into_iter()).collect();

        self.transactions.requeue_transactions(transactions);

        tracing::warn!(%batch, descendents=?batches, "Batch failed, dropping all inflight descendent batches, impacted transactions are back in queue.");
    }

    /// Marks a batch as proven if it exists.
    pub fn batch_proved(&mut self, batch_id: BatchId) {
        self.batches.mark_proven(batch_id);
    }

    /// Select at most `count` batches which are ready to be placed into the next block.
    ///
    /// May return an empty batch set if no batches are ready.
    pub fn select_block(&mut self, count: usize) -> (BlockNumber, Vec<BatchId>) {
        // TODO: should return actual batch transaction data as well.

        let mut batches = Vec::with_capacity(count);
        for _ in 0..count {
            let Some((batch_id, _)) = self.batches.pop_for_blocking() else {
                break;
            };

            batches.push(batch_id);

            // Unlike `select_batch` we don't need to track inter-block depedencies as this
            // relationship is inherently sequential.
        }

        let block_number = self.next_inflight_block;
        self.next_inflight_block.increment();
        self.block_pool.insert(block_number, batches.clone());

        (block_number, batches)
    }

    /// Notify the pool that the block was succesfully completed.
    ///
    /// Panics if blocks are completed out-of-order. todo: might be a better way, but this is pretty
    /// unrecoverable..
    pub fn block_completed(&mut self, block_number: BlockNumber) {
        assert_eq!(block_number, self.completed_blocks, "Blocks must be submitted sequentially");

        // Update book keeping by removing the inflight data that just became stale.
        self.completed_blocks.increment();

        let Some(stale_block) = self.stale_block() else {
            return;
        };

        let stale_batches = self.block_pool.remove(&stale_block).expect("Block should be in graph");

        let stale_transations = self.batches.remove_stale(stale_batches);
        self.transactions.removed_stale(stale_transations);
    }

    pub fn block_failed(&mut self, block: BlockNumber) {
        // TBD.. not quite sure what to do here yet. Presumably the caller has already retried this
        // block so the block is just inherently broken.
        //
        // Given lack of information at this stage we should probably just abort the node?
        // In the future we might improve the situation with more fine-grained failure reasons.
    }

    /// The highest block height we consider stale.
    ///
    /// Returns [None] if the blockchain is so short that all blocks are considered fresh.
    fn stale_block(&self) -> Option<BlockNumber> {
        self.completed_blocks.checked_sub(self.staleness)
    }
}

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum AddTransactionError {
    #[error("Transaction's initial account state {expected} did not match the current account state {current}.")]
    InvalidAccountState { current: Digest, expected: Digest },
    #[error("Transaction input data is stale. Required data fresher than {stale_limit} but inputs are from {input_block}.")]
    StaleInputs {
        input_block: BlockNumber,
        stale_limit: BlockNumber,
    },
}
