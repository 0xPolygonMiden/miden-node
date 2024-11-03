use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
};

use batch_graph::BatchGraph;
use inflight_state::InflightState;
use transaction_graph::TransactionGraph;

use crate::{
    batch_builder::batch::TransactionBatch, domain::transaction::AuthenticatedTransaction,
    errors::AddTransactionError,
};

mod batch_graph;
mod dependency_graph;
mod inflight_state;
mod transaction_graph;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BatchJobId(u64);

impl Display for BatchJobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl BatchJobId {
    pub fn increment(&mut self) {
        self.0 += 1;
    }

    #[cfg(test)]
    pub fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockNumber(u32);

impl Display for BlockNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl BlockNumber {
    pub fn new(x: u32) -> Self {
        Self(x)
    }

    pub fn next(&self) -> Self {
        let mut ret = *self;
        ret.increment();

        ret
    }

    pub fn prev(&self) -> Option<Self> {
        self.checked_sub(Self(1))
    }

    pub fn increment(&mut self) {
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
    transactions: TransactionGraph,

    /// Inflight batches.
    batches: BatchGraph,

    /// The next batches ID.
    next_batch_id: BatchJobId,

    /// The current block height of the chain.
    chain_tip: BlockNumber,

    block_in_progress: Option<BTreeSet<BatchJobId>>,

    batch_transaction_limit: usize,
    block_batch_limit: usize,
}

impl Mempool {
    /// Creates a new [Mempool] with the provided configuration.
    pub fn new(
        chain_tip: BlockNumber,
        batch_limit: usize,
        block_limit: usize,
        state_retention: usize,
    ) -> Self {
        Self {
            chain_tip,
            batch_transaction_limit: batch_limit,
            block_batch_limit: block_limit,
            state: InflightState::new(chain_tip, state_retention),
            block_in_progress: Default::default(),
            transactions: Default::default(),
            batches: Default::default(),
            next_batch_id: Default::default(),
        }
    }

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
        // Add transaction to inflight state.
        let parents = self.state.add_transaction(&transaction)?;

        self.transactions.insert(transaction, parents).expect("Malformed graph");

        Ok(self.chain_tip.0)
    }

    /// Returns a set of transactions for the next batch.
    ///
    /// Transactions are returned in a valid execution ordering.
    ///
    /// Returns `None` if no transactions are available.
    pub fn select_batch(&mut self) -> Option<(BatchJobId, Vec<AuthenticatedTransaction>)> {
        let (batch, parents) = self.transactions.select_batch(self.batch_transaction_limit);
        if batch.is_empty() {
            return None;
        }
        let tx_ids = batch.iter().map(AuthenticatedTransaction::id).collect();

        let batch_id = self.next_batch_id;
        self.next_batch_id.increment();

        self.batches.insert(batch_id, tx_ids, parents).expect("Malformed graph");

        Some((batch_id, batch))
    }

    /// Drops the failed batch and all of its descendants.
    ///
    /// Transactions are placed back in the queue.
    pub fn batch_failed(&mut self, batch: BatchJobId) {
        let removed_batches =
            self.batches.remove_batches([batch].into()).expect("Batch was not present");

        // Its possible to receive failures for batches which were already removed
        // as part of a prior failure. Early exit to prevent logging these no-ops.
        if removed_batches.is_empty() {
            return;
        }

        let batches = removed_batches.keys().copied().collect::<Vec<_>>();
        let transactions = removed_batches.into_values().flatten().collect();

        self.transactions.requeue_transactions(transactions).expect("Malformed graph");

        tracing::warn!(%batch, descendents=?batches, "Batch failed, dropping all inflight descendent batches, impacted transactions are back in queue.");
    }

    /// Marks a batch as proven if it exists.
    pub fn batch_proved(&mut self, batch_id: BatchJobId, batch: TransactionBatch) {
        self.batches.submit_proof(batch_id, batch).expect("Malformed graph");
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
        let transactions = self.batches.prune_committed(batches).expect("Batches failed to commit");
        self.transactions
            .commit_transactions(&transactions)
            .expect("Transaction graph malformed");

        // Inform inflight state about committed data.
        self.state.commit_block(transactions);

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
        let purged = self.batches.remove_batches(batches).expect("Bad graph");
        let transactions = purged.into_values().flatten().collect();

        let transactions = self
            .transactions
            .remove_transactions(transactions)
            .expect("Transaction graph is malformed");

        // Rollback state.
        self.state.revert_transactions(transactions);
    }
}
