use std::{collections::BTreeSet, fmt::Display, sync::Arc};

use batch_graph::BatchGraph;
use graph::GraphError;
use inflight_state::InflightState;
use miden_objects::{
    transaction::TransactionId, MAX_ACCOUNTS_PER_BATCH, MAX_INPUT_NOTES_PER_BATCH,
    MAX_OUTPUT_NOTES_PER_BATCH,
};
use tokio::sync::Mutex;
use tracing::instrument;
use transaction_expiration::TransactionExpirations;
use transaction_graph::TransactionGraph;

use crate::{
    batch_builder::batch::{BatchId, TransactionBatch},
    domain::transaction::AuthenticatedTransaction,
    errors::AddTransactionError,
    COMPONENT, SERVER_MAX_BATCHES_PER_BLOCK, SERVER_MAX_TXS_PER_BATCH,
};

mod batch_graph;
mod graph;
mod inflight_state;
mod transaction_expiration;
mod transaction_graph;

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

    pub fn into_inner(self) -> u32 {
        self.0
    }
}

// MEMPOOL BUDGET
// ================================================================================================

/// Limits placed on a batch's contents.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

        let output_notes = tx.output_note_count();
        let input_notes = tx.input_note_count();

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
    fn check_then_subtract(&mut self, _batch: &TransactionBatch) -> BudgetStatus {
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

#[derive(Clone, Debug, PartialEq)]
pub struct Mempool {
    /// The latest inflight state of each account.
    ///
    /// Accounts without inflight transactions are not stored.
    state: InflightState,

    /// Inflight transactions.
    transactions: TransactionGraph,

    expirations: TransactionExpirations,

    /// Inflight batches.
    batches: BatchGraph,

    /// The current block height of the chain.
    chain_tip: BlockNumber,

    /// The current inflight block, if any.
    block_in_progress: Option<BTreeSet<BatchId>>,

    block_budget: BlockBudget,
    batch_budget: BatchBudget,
}

impl Mempool {
    /// Creates a new [SharedMempool] with the provided configuration.
    pub fn shared(
        chain_tip: BlockNumber,
        batch_budget: BatchBudget,
        block_budget: BlockBudget,
        state_retention: usize,
    ) -> SharedMempool {
        Arc::new(Mutex::new(Self::new(chain_tip, batch_budget, block_budget, state_retention)))
    }

    fn new(
        chain_tip: BlockNumber,
        batch_budget: BatchBudget,
        block_budget: BlockBudget,
        state_retention: usize,
    ) -> Mempool {
        Self {
            chain_tip,
            batch_budget,
            block_budget,
            state: InflightState::new(chain_tip, state_retention),
            block_in_progress: Default::default(),
            transactions: Default::default(),
            batches: Default::default(),
            expirations: Default::default(),
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
    #[instrument(target = COMPONENT, skip_all, fields(tx=%transaction.id()))]
    pub fn add_transaction(
        &mut self,
        transaction: AuthenticatedTransaction,
    ) -> Result<u32, AddTransactionError> {
        // Add transaction to inflight state.
        let parents = self.state.add_transaction(&transaction)?;

        self.expirations.insert(transaction.id(), transaction.expires_at());

        self.transactions
            .insert(transaction, parents)
            .expect("Transaction should insert after passing inflight state");

        Ok(self.chain_tip.0)
    }

    /// Returns a set of transactions for the next batch.
    ///
    /// Transactions are returned in a valid execution ordering.
    ///
    /// Returns `None` if no transactions are available.
    #[instrument(target = COMPONENT, skip_all)]
    pub fn select_batch(&mut self) -> Option<(BatchId, Vec<AuthenticatedTransaction>)> {
        let (batch, parents) = self.transactions.select_batch(self.batch_budget);
        if batch.is_empty() {
            return None;
        }
        let tx_ids = batch.iter().map(AuthenticatedTransaction::id).collect::<Vec<_>>();

        let batch_id = self.batches.insert(tx_ids, parents).expect("Selected batch should insert");

        Some((batch_id, batch))
    }

    /// Drops the failed batch and all of its descendants.
    ///
    /// Transactions are placed back in the queue.
    #[instrument(target = COMPONENT, skip_all, fields(batch))]
    pub fn batch_failed(&mut self, batch: BatchId) {
        // Batch may already have been removed as part of a parent batches failure.
        if !self.batches.contains(&batch) {
            return;
        }

        let removed_batches =
            self.batches.remove_batches([batch].into()).expect("Batch was not present");

        let transactions = removed_batches.values().flatten().copied().collect();

        self.transactions
            .requeue_transactions(transactions)
            .expect("Transaction should requeue");

        tracing::warn!(
            %batch,
            descendents=?removed_batches.keys(),
            "Batch failed, dropping all inflight descendent batches, impacted transactions are back in queue."
        );
    }

    /// Marks a batch as proven if it exists.
    #[instrument(target = COMPONENT, skip_all, fields(batch=%batch.id()))]
    pub fn batch_proved(&mut self, batch: TransactionBatch) {
        // Batch may have been removed as part of a parent batches failure.
        if !self.batches.contains(&batch.id()) {
            return;
        }

        self.batches.submit_proof(batch).expect("Batch proof should submit");
    }

    /// Select batches for the next block.
    ///
    /// Note that the set of batches
    /// - may be empty if none are available, and
    /// - may contain dependencies and therefore the order must be maintained
    ///
    /// # Panics
    ///
    /// Panics if there is already a block in flight.
    #[instrument(target = COMPONENT, skip_all)]
    pub fn select_block(&mut self) -> (BlockNumber, Vec<TransactionBatch>) {
        assert!(self.block_in_progress.is_none(), "Cannot have two blocks inflight.");

        let batches = self.batches.select_block(self.block_budget);
        self.block_in_progress = Some(batches.iter().map(TransactionBatch::id).collect());

        (self.chain_tip.next(), batches)
    }

    /// Notify the pool that the block was successfully completed.
    ///
    /// # Panics
    ///
    /// Panics if blocks are completed out-of-order or if there is no block in flight.
    #[instrument(target = COMPONENT, skip_all, fields(block_number))]
    pub fn block_committed(&mut self, block_number: BlockNumber) {
        assert_eq!(block_number, self.chain_tip.next(), "Blocks must be submitted sequentially");

        // Remove committed batches and transactions from graphs.
        let batches = self.block_in_progress.take().expect("No block in progress to commit");
        let transactions = self.batches.prune_committed(batches).expect("Batches failed to commit");
        self.transactions
            .commit_transactions(&transactions)
            .expect("Transaction graph malformed");

        // Remove the committed transactions from expiration tracking.
        self.expirations.remove(transactions.iter());

        // Inform inflight state about committed data.
        self.state.commit_block(transactions);
        self.chain_tip.increment();

        // Revert expired transactions and their descendents.
        let expired = self.expirations.get(block_number);
        self.revert_transactions(expired.into_iter().collect())
            .expect("expired transactions must be part of the mempool");
    }

    /// Block and all of its contents and dependents are purged from the mempool.
    ///
    /// # Panics
    ///
    /// Panics if there is no block in flight or if the block number does not match the current
    /// inflight block.
    #[instrument(target = COMPONENT, skip_all, fields(block_number))]
    pub fn block_failed(&mut self, block_number: BlockNumber) {
        assert_eq!(block_number, self.chain_tip.next(), "Blocks must be submitted sequentially");

        let batches = self.block_in_progress.take().expect("No block in progress to be failed");

        // Revert all transactions. This is the nuclear (but simplest) solution.
        //
        // We currently don't have a way of determining why this block failed so take the safe route
        // and just nuke all associated transactions.
        //
        // TODO: improve this strategy, e.g. count txn failures (as well as in e.g. batch failures),
        // and only revert upon exceeding some threshold.
        let txs = batches
            .into_iter()
            .flat_map(|batch| {
                self.batches
                    .get_transactions(&batch)
                    .expect("batch from a block must be in the mempool")
            })
            .copied()
            .collect();
        self.revert_transactions(txs)
            .expect("transactions from a block must be part of the mempool");
    }

    /// Reverts the given transactions and their descendents from the mempool.
    ///
    /// This includes removing them from the transaction and batch graphs, as well as cleaning up
    /// their inflight state and expiration mappings.
    ///
    /// Transactions that were in reverted batches but that are disjoint from the reverted
    /// transactions (i.e. not descendents) are requeued and _not_ reverted.
    ///
    /// # Errors
    ///
    /// Returns an error if any transaction was not in the transaction graph i.e. if the transaction
    /// is unknown.
    fn revert_transactions(
        &mut self,
        txs: Vec<TransactionId>,
    ) -> Result<(), GraphError<TransactionId>> {
        // Revert all transactions and their descendents, and their associated batches.
        let reverted = self.transactions.remove_transactions(txs)?;
        let batches_reverted = self.batches.remove_transactions(reverted.iter());

        // Requeue transactions that are disjoint from the reverted set, but were part of the
        // reverted batches.
        let to_requeue = batches_reverted
            .into_values()
            .flatten()
            .filter(|tx| !reverted.contains(tx))
            .collect();
        self.transactions
            .requeue_transactions(to_requeue)
            .expect("transactions from batches must be requeueable");

        // Cleanup state.
        self.expirations.remove(reverted.iter());
        self.state.revert_transactions(reverted);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::test_utils::MockProvenTxBuilder;

    impl Mempool {
        fn for_tests() -> Self {
            Self::new(BlockNumber::new(0), Default::default(), Default::default(), 5)
        }
    }

    // BATCH FAILED TESTS
    // ================================================================================================

    #[test]
    fn children_of_failed_batches_are_ignored() {
        // Batches are proved concurrently. This makes it possible for a child job to complete after
        // the parent has been reverted (and therefore reverting the child job). Such a child job
        // should be ignored.
        let txs = MockProvenTxBuilder::sequential();

        let mut uut = Mempool::for_tests();
        uut.add_transaction(txs[0].clone()).unwrap();
        let (parent_batch, batch_txs) = uut.select_batch().unwrap();
        assert_eq!(batch_txs, vec![txs[0].clone()]);

        uut.add_transaction(txs[1].clone()).unwrap();
        let (child_batch_a, batch_txs) = uut.select_batch().unwrap();
        assert_eq!(batch_txs, vec![txs[1].clone()]);

        uut.add_transaction(txs[2].clone()).unwrap();
        let (_, batch_txs) = uut.select_batch().unwrap();
        assert_eq!(batch_txs, vec![txs[2].clone()]);

        // Child batch jobs are now dangling.
        uut.batch_failed(parent_batch);
        let reference = uut.clone();

        // Success or failure of the child job should effectively do nothing.
        uut.batch_failed(child_batch_a);
        assert_eq!(uut, reference);

        let proof =
            TransactionBatch::new([txs[2].raw_proven_transaction()], Default::default()).unwrap();
        uut.batch_proved(proof);
        assert_eq!(uut, reference);
    }

    #[test]
    fn failed_batch_transactions_are_requeued() {
        let txs = MockProvenTxBuilder::sequential();

        let mut uut = Mempool::for_tests();
        uut.add_transaction(txs[0].clone()).unwrap();
        uut.select_batch().unwrap();

        uut.add_transaction(txs[1].clone()).unwrap();
        let (failed_batch, _) = uut.select_batch().unwrap();

        uut.add_transaction(txs[2].clone()).unwrap();
        uut.select_batch().unwrap();

        // Middle batch failed, so it and its child transaction should be re-entered into the queue.
        uut.batch_failed(failed_batch);

        let mut reference = Mempool::for_tests();
        reference.add_transaction(txs[0].clone()).unwrap();
        reference.select_batch().unwrap();
        reference.add_transaction(txs[1].clone()).unwrap();
        reference.add_transaction(txs[2].clone()).unwrap();

        assert_eq!(uut, reference);
    }

    // BLOCK COMMITTED TESTS
    // ================================================================================================

    /// Expired transactions should be reverted once their expiration block is committed.
    #[test]
    fn block_commit_reverts_expired_txns() {
        let mut uut = Mempool::for_tests();

        let tx_to_commit = MockProvenTxBuilder::with_account_index(0).build();
        let tx_to_commit = AuthenticatedTransaction::from_inner(tx_to_commit);

        // Force the tx into a pending block.
        uut.add_transaction(tx_to_commit.clone()).unwrap();
        uut.select_batch().unwrap();
        uut.batch_proved(
            TransactionBatch::new([tx_to_commit.raw_proven_transaction()], Default::default())
                .unwrap(),
        );
        let (block, _) = uut.select_block();
        // A reverted transaction behaves as if it never existed, the current state is the expected
        // outcome, plus an extra committed block at the end.
        let mut reference = uut.clone();

        // Add a new transaction which will expire when the pending block is committed.
        let tx_to_revert = MockProvenTxBuilder::with_account_index(1)
            .expiration_block_num(block.into_inner())
            .build();
        let tx_to_revert = AuthenticatedTransaction::from_inner(tx_to_revert);
        uut.add_transaction(tx_to_revert).unwrap();

        // Commit the pending block which should revert the above tx.
        uut.block_committed(block);
        reference.block_committed(block);

        assert_eq!(uut, reference);
    }

    #[test]
    fn empty_block_commitment() {
        let mut uut = Mempool::for_tests();

        for _ in 0..3 {
            let (block, _) = uut.select_block();
            uut.block_committed(block);
        }
    }

    #[test]
    #[should_panic]
    fn blocks_must_be_committed_sequentially() {
        let mut uut = Mempool::for_tests();

        let (block, _) = uut.select_block();
        uut.block_committed(block.next());
    }

    #[test]
    #[should_panic]
    fn block_commitment_is_rejected_if_no_block_is_in_flight() {
        Mempool::for_tests().block_committed(BlockNumber::new(1));
    }

    #[test]
    #[should_panic]
    fn cannot_have_multple_inflight_blocks() {
        let mut uut = Mempool::for_tests();

        uut.select_block();
        uut.select_block();
    }

    // BLOCK FAILED TESTS
    // ================================================================================================

    /// A failed block should have all of its transactions reverted.
    #[test]
    fn block_failure_reverts_its_transactions() {
        let mut uut = Mempool::for_tests();
        // We will revert everything so the reference should be the empty mempool.
        let reference = uut.clone();

        let reverted_txs = MockProvenTxBuilder::sequential();

        uut.add_transaction(reverted_txs[0].clone()).unwrap();
        uut.select_batch().unwrap();
        uut.batch_proved(
            TransactionBatch::new([reverted_txs[0].raw_proven_transaction()], Default::default())
                .unwrap(),
        );

        // Block 1 will contain just the first batch.
        let (block_number, _) = uut.select_block();

        // Create another dependent batch.
        uut.add_transaction(reverted_txs[1].clone()).unwrap();
        uut.select_batch();
        // Create another dependent transaction.
        uut.add_transaction(reverted_txs[2].clone()).unwrap();

        // Fail the block which should result in everything reverting.
        uut.block_failed(block_number);

        assert_eq!(uut, reference);
    }

    // TRANSACTION REVERSION TESTS
    // ================================================================================================

    /// Ensures that reverting transactions is equivalent to them never being inserted at all.
    ///
    /// This checks that there are no forgotten links to them exist anywhere in the mempool by
    /// comparing to a reference mempool that never had them inserted.
    #[test]
    fn reverted_transactions_and_descendents_are_non_existent() {
        let mut uut = Mempool::for_tests();

        let reverted_txs = MockProvenTxBuilder::sequential();

        uut.add_transaction(reverted_txs[0].clone()).unwrap();
        uut.select_batch().unwrap();

        uut.add_transaction(reverted_txs[1].clone()).unwrap();
        uut.select_batch().unwrap();

        uut.add_transaction(reverted_txs[2].clone()).unwrap();
        uut.revert_transactions(vec![reverted_txs[1].id()]).unwrap();

        // We expect the second batch and the latter reverted txns to be non-existent.
        let mut reference = Mempool::for_tests();
        reference.add_transaction(reverted_txs[0].clone()).unwrap();
        reference.select_batch().unwrap();

        assert_eq!(uut, reference);
    }

    /// Reverting transactions causes their batches to also revert. These batches in turn contain
    /// non-reverted transactions which should be requeued (and not reverted).
    #[test]
    fn reverted_transaction_batches_are_requeued() {
        let mut uut = Mempool::for_tests();

        let unrelated_txs = MockProvenTxBuilder::sequential();
        let reverted_txs = MockProvenTxBuilder::sequential();

        uut.add_transaction(reverted_txs[0].clone()).unwrap();
        uut.add_transaction(unrelated_txs[0].clone()).unwrap();
        uut.select_batch().unwrap();

        uut.add_transaction(reverted_txs[1].clone()).unwrap();
        uut.add_transaction(unrelated_txs[1].clone()).unwrap();
        uut.select_batch().unwrap();

        uut.add_transaction(reverted_txs[2].clone()).unwrap();
        uut.add_transaction(unrelated_txs[2].clone()).unwrap();
        uut.revert_transactions(vec![reverted_txs[1].id()]).unwrap();

        // We expect the second batch and the latter reverted txns to be non-existent.
        let mut reference = Mempool::for_tests();
        reference.add_transaction(reverted_txs[0].clone()).unwrap();
        reference.add_transaction(unrelated_txs[0].clone()).unwrap();
        reference.select_batch().unwrap();
        reference.add_transaction(unrelated_txs[1].clone()).unwrap();
        reference.add_transaction(unrelated_txs[2].clone()).unwrap();

        assert_eq!(uut, reference);
    }
}
