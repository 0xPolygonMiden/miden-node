use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    sync::Arc,
};

use batch_graph::BatchGraph;
use inflight_state::InflightState;
use miden_objects::{
    MAX_ACCOUNTS_PER_BATCH, MAX_INPUT_NOTES_PER_BATCH, MAX_OUTPUT_NOTES_PER_BATCH,
};
use tokio::sync::Mutex;
use transaction_graph::TransactionGraph;

use crate::{
    batch_builder::batch::TransactionBatch, domain::transaction::AuthenticatedTransaction,
    errors::AddTransactionError, SERVER_MAX_BATCHES_PER_BLOCK, SERVER_MAX_TXS_PER_BATCH,
    SERVER_MEMPOOL_STATE_RETENTION,
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

// MEMPOOL BUDGET
// ================================================================================================

/// Limits placed on a batch's contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchBudget {
    /// Maximum number of transactions allowed in a batch.
    transactions: usize,
    /// Maximum number of input notes allowed.
    input_notes: usize,
    /// Maximum number of output notes allowed.
    output_notes: usize,
    /// Maximum number of updated accounts.
    accounts: usize,
}

/// Limits placed on a blocks's contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockBudget {
    /// Maximum number of batches allowed in a block.
    batches: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BudgetStatus {
    /// The operation remained within the budget.
    WithinScope,
    /// The operation exceeded the budget.
    Exceeded,
}

impl Default for BatchBudget {
    fn default() -> Self {
        Self {
            transactions: SERVER_MAX_TXS_PER_BATCH,
            input_notes: MAX_INPUT_NOTES_PER_BATCH,
            output_notes: MAX_OUTPUT_NOTES_PER_BATCH,
            accounts: MAX_ACCOUNTS_PER_BATCH,
        }
    }
}

impl Default for BlockBudget {
    fn default() -> Self {
        Self { batches: SERVER_MAX_BATCHES_PER_BLOCK }
    }
}

impl BatchBudget {
    /// Attempts to consume the transaction's resources from the budget.
    ///
    /// Returns [BudgetStatus::Exceeded] if the transaction would exceed the remaining budget,
    /// otherwise returns [BudgetStatus::Ok] and subtracts the resources from the budger.
    #[must_use]
    fn check_then_subtract(&mut self, tx: &AuthenticatedTransaction) -> BudgetStatus {
        // This type assertion reminds us to update the account check if we ever support multiple
        // account updates per tx.
        let _: miden_objects::accounts::AccountId = tx.account_update().account_id();
        const ACCOUNT_UPDATES_PER_TX: usize = 1;

        // TODO: This is inefficient and ProvenTransaction should provide len() access.
        let output_notes = tx.output_notes().count();
        let input_notes = tx.nullifiers().count();

        if self.transactions == 0
            || self.accounts < ACCOUNT_UPDATES_PER_TX
            || self.input_notes < input_notes
            || self.output_notes < output_notes
        {
            return BudgetStatus::Exceeded;
        }

        self.transactions -= 1;
        self.accounts -= ACCOUNT_UPDATES_PER_TX;
        self.input_notes -= input_notes;
        self.output_notes -= output_notes;

        BudgetStatus::WithinScope
    }
}

impl BlockBudget {
    /// Attempts to consume the batch's resources from the budget.
    ///
    /// Returns [BudgetStatus::Exceeded] if the batch would exceed the remaining budget,
    /// otherwise returns [BudgetStatus::Ok].
    #[must_use]
    fn check_then_deplete(&mut self, _batch: &TransactionBatch) -> BudgetStatus {
        if self.batches == 0 {
            BudgetStatus::Exceeded
        } else {
            self.batches -= 1;
            BudgetStatus::WithinScope
        }
    }
}

// MEMPOOL
// ================================================================================================

pub type SharedMempool = Arc<Mutex<Mempool>>;

#[derive(Clone)]
pub struct MempoolBuilder {
    /// Limits placed on each batch.
    pub batch_limits: BatchBudget,
    /// The maximum number of batches that will be selected for a block.
    pub block_limits: BlockBudget,
    /// Number of committed blocks the mempool will retain in its state tracking.
    pub committed_state_retention: usize,
}

impl Default for MempoolBuilder {
    fn default() -> Self {
        Self {
            committed_state_retention: SERVER_MEMPOOL_STATE_RETENTION,
            block_limits: Default::default(),
            batch_limits: Default::default(),
        }
    }
}

impl MempoolBuilder {
    pub fn build_shared(self, chain_tip: BlockNumber) -> SharedMempool {
        SharedMempool::new(self.build(chain_tip))
    }

    fn build(self, chain_tip: BlockNumber) -> Mempool {
        let Self {
            block_limits,
            committed_state_retention,
            batch_limits,
        } = self;
        Mempool {
            chain_tip,
            block_limits,
            batch_limits,
            state: InflightState::new(chain_tip, committed_state_retention),
            block_in_progress: Default::default(),
            transactions: Default::default(),
            batches: Default::default(),
            next_batch_id: Default::default(),
        }
    }
}

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

    batch_limits: BatchBudget,

    block_limits: BlockBudget,
}

impl Mempool {
    /// Creates a new [Mempool] with the provided configuration.
    pub fn new(
        chain_tip: BlockNumber,
        batch_limit: usize,
        block_limit: usize,
        state_retention: usize,
    ) -> SharedMempool {
        Arc::new(Mutex::new(Self {
            chain_tip,
            batch_transaction_limit: batch_limit,
            block_batch_limit: block_limit,
            state: InflightState::new(chain_tip, state_retention),
            block_in_progress: Default::default(),
            transactions: Default::default(),
            batches: Default::default(),
            next_batch_id: Default::default(),
        }))
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
        let (batch, parents) = self.transactions.select_batch(self.batch_limits.clone());
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

        let batches = self.batches.select_block(self.block_limits.clone());
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
