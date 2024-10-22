use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque},
    fmt::Display,
    ops::Sub,
    sync::Arc,
};

use batch_graph::BatchGraph;
use inflight_state::{InflightState, StateDelta};
use miden_objects::{
    accounts::AccountId,
    notes::{NoteId, Nullifier},
    transaction::{ProvenTransaction, TransactionId},
    Digest,
};
use miden_tx::{utils::collections::KvMap, TransactionVerifierError};
use transaction_graph::TransactionGraph;

use crate::{
    batch_builder::batch::TransactionBatch,
    errors::{AddTransactionError, VerifyTxError},
    store::{TransactionInputs, TxInputsError},
    transaction::AuthenticatedTransaction,
};

mod batch_graph;
mod inflight_state;
mod transaction_graph;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BatchJobId(u64);

impl Display for BatchJobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl BatchJobId {
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
    pub fn next(&self) -> Self {
        let mut ret = *self;
        ret.increment();

        ret
    }

    pub fn prev(&self) -> Option<Self> {
        self.checked_sub(Self(1))
    }

    pub fn increment(mut self) {
        self.0 += 1;
    }

    pub fn checked_sub(&self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }
}

// MEMPOOL
// ================================================================================================

pub struct Mempool {
    /// The latest inflight state of each account.
    ///
    /// Accounts without inflight transactions are not stored.
    state: InflightState,

    /// Inflight transactions.
    transactions: TransactionGraph<AuthenticatedTransaction>,

    /// Inflight batches.
    batches: BatchGraph,

    /// The next batches ID.
    next_batch_id: BatchJobId,

    /// Blocks which have been committed but are not yet considered stale.
    committed_diffs: VecDeque<StateDelta>,

    /// The current block height of the chain.
    chain_tip: BlockNumber,

    block_in_progress: Option<BTreeSet<BatchJobId>>,

    /// Amount of recently committed blocks we retain in addition to the inflight state.
    ///
    /// This provides an overlap between committed and inflight state, giving a grace
    /// period for incoming transactions to be verified against both without requiring it
    /// to be an atomic action.
    num_retained_blocks: BlockNumber,

    batch_transaction_limit: usize,
    block_batch_limit: usize,
}

impl Mempool {
    /// Adds a transaction to the mempool.
    ///
    /// # Returns
    ///
    /// Returns the current block height.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction's initial conditions don't match the current state.
    pub fn add_transaction(
        &mut self,
        transaction: AuthenticatedTransaction,
    ) -> Result<u32, AddTransactionError> {
        // The mempool retains recently committed blocks, in addition to the state that is currently inflight.
        // This overlap with the committed state allows us to verify incoming transactions against the current
        // state (committed + inflight). Transactions are first authenticated against the committed state prior
        // to being submitted to the mempool. The overlap provides a grace period between transaction authentication
        // against committed state and verification against inflight state.
        //
        // Here we just ensure that this authentication point is still within this overlap zone. This should only fail
        // if the grace period is too restrictive for the current combination of block rate, transaction throughput and
        // database IO.
        if let Some(stale_block) = self.stale_block() {
            if transaction.authentication_height() <= stale_block.0 {
                return Err(AddTransactionError::StaleInputs {
                    input_block: BlockNumber(transaction.authentication_height()),
                    stale_limit: stale_block,
                });
            }
        }

        // Add transaction to inflight state.
        let parents = self.state.add_transaction(&transaction)?;

        self.transactions.insert(transaction.id(), transaction, parents);

        Ok(self.chain_tip.0)
    }

    /// Returns a set of transactions for the next batch.
    ///
    /// Transactions are returned in a valid execution ordering.
    ///
    /// Returns `None` if no transactions are available.
    pub fn select_batch(&mut self) -> Option<(BatchJobId, Vec<AuthenticatedTransaction>)> {
        let mut parents = BTreeSet::new();
        let mut batch = Vec::with_capacity(self.batch_transaction_limit);
        let mut tx_ids = Vec::with_capacity(self.batch_transaction_limit);

        for _ in 0..self.batch_transaction_limit {
            // Select transactions according to some strategy here. For now its just arbitrary.
            let Some((tx, tx_parents)) = self.transactions.pop_for_processing() else {
                break;
            };
            batch.push(tx);
            parents.extend(tx_parents);
        }

        // Update the dependency graph to reflect parents at the batch level by removing all edges
        // within this batch.
        for tx in &batch {
            parents.remove(&tx.id());
        }

        let batch_id = self.next_batch_id;
        self.next_batch_id.increment();

        self.batches.insert(batch_id, tx_ids, parents);

        Some((batch_id, batch))
    }

    /// Drops the failed batch and all of its descendants.
    ///
    /// Transactions are placed back in the queue.
    pub fn batch_failed(&mut self, batch: BatchJobId) {
        let removed_batches = self.batches.purge_subgraphs([batch].into());

        // Its possible to receive failures for batches which were already removed
        // as part of a prior failure. Early exit to prevent logging these no-ops.
        if removed_batches.is_empty() {
            return;
        }

        let batches = removed_batches.keys().copied().collect::<Vec<_>>();
        let transactions = removed_batches.into_values().flatten().collect();

        self.transactions.requeue_transactions(transactions);

        tracing::warn!(%batch, descendents=?batches, "Batch failed, dropping all inflight descendent batches, impacted transactions are back in queue.");
    }

    /// Marks a batch as proven if it exists.
    pub fn batch_proved(&mut self, batch_id: BatchJobId, batch: TransactionBatch) {
        self.batches.mark_proven(batch_id, batch);
    }

    /// Select batches for the next block.
    ///
    /// May return an empty set if no batches are ready.
    ///
    /// # Panics
    ///
    /// Panics if there is already a block in flight.
    pub fn select_block(&mut self) -> (BlockNumber, BTreeMap<BatchJobId, TransactionBatch>) {
        assert!(self.block_in_progress.is_none(), "Cannot have two blocks inflight.");

        let batches = self.batches.select_block(self.block_batch_limit);
        self.block_in_progress = Some(batches.keys().cloned().collect());

        (self.chain_tip.next(), batches)
    }

    /// Notify the pool that the block was successfully completed.
    ///
    /// # Panics
    ///
    /// Panics if blocks are completed out-of-order or if there is no block in flight.
    pub fn block_committed(&mut self, block_number: BlockNumber) {
        assert_eq!(block_number, self.chain_tip.next(), "Blocks must be submitted sequentially");

        // Remove committed batches and transactions from graphs.
        let batches = self.block_in_progress.take().expect("No block in progress to commit");
        let transactions = self.batches.remove_committed(batches);
        let transactions = self.transactions.prune_processed(&transactions);

        // Inform inflight state about committed data.
        let diff = StateDelta::new(&transactions);
        self.state.commit_transactions(&diff);

        self.committed_diffs.push_back(diff);
        if self.committed_diffs.len() > self.num_retained_blocks.0 as usize {
            // SAFETY: just checked that length is non-zero.
            let stale_block = self.committed_diffs.pop_front().unwrap();

            self.state.prune_committed_state(stale_block);
        }

        self.chain_tip.increment();
    }

    /// Block and all of its contents and dependents are purged from the mempool.
    ///
    /// # Panics
    ///
    /// Panics if there is no block in flight or if the block number does not match the current
    /// inflight block.
    pub fn block_failed(&mut self, block_number: BlockNumber) {
        assert_eq!(block_number, self.chain_tip.next(), "Blocks must be submitted sequentially");

        let batches = self.block_in_progress.take().expect("No block in progress to be failed");

        // Remove all transactions from the graphs.
        let purged = self.batches.purge_subgraphs(batches);
        let batches = purged.keys().collect::<Vec<_>>();
        let transactions = purged.into_values().flatten().collect();

        let transactions = self.transactions.purge_subgraphs(transactions);

        // Rollback state.
        self.state.revert_transactions(&transactions);
    }

    /// The highest block height we consider stale.
    ///
    /// Returns [None] if the blockchain is so short that all blocks are considered fresh.
    fn stale_block(&self) -> Option<BlockNumber> {
        self.chain_tip.checked_sub(self.num_retained_blocks)
    }
}
