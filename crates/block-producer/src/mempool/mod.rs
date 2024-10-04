#![allow(unused)]

use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque},
    fmt::Display,
    ops::Sub,
    sync::Arc,
};

use account_state::InflightState;
use batch_graph::BatchGraph;
use miden_objects::{
    accounts::AccountId,
    notes::{NoteId, Nullifier},
    transaction::{ProvenTransaction, TransactionId},
    Digest,
};
use miden_tx::{utils::collections::KvMap, TransactionVerifierError};
use transaction_graph::TransactionGraph;

use crate::store::{TransactionInputs, TxInputsError};

mod account_state;
mod batch_graph;
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

pub struct Mempool {
    /// The latest inflight state of each account.
    ///
    /// Accounts without inflight transactions are not stored.
    state: InflightState,

    /// Note's consumed by inflight transactions.
    nullifiers: BTreeSet<Nullifier>,

    /// Notes produced by inflight transactions.
    ///
    /// It is possible for these to already be consumed - check nullifiers.
    notes: BTreeMap<NoteId, TransactionId>,

    /// Inflight transactions.
    transactions: TransactionGraph,

    /// Inflight batches.
    batches: BatchGraph,

    /// The next batches ID.
    next_batch_id: BatchJobId,

    /// Blocks which have been committed but are not yet considered stale.
    committed_blocks: VecDeque<StateDiff>,

    /// The current block height of the chain.
    chain_tip: BlockNumber,

    block_in_progress: Option<BTreeSet<BatchJobId>>,

    /// Number of blocks before transaction input is considered stale.
    staleness: BlockNumber,

    batch_transaction_limit: usize,
    block_batch_limit: usize,
}

impl Mempool {
    /// Complete barring todos.
    pub fn add_transaction(
        &mut self,
        transaction: ProvenTransaction,
        inputs: TransactionInputs,
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

        // Add transaction to inflight state.
        let parents = self.state.add_transaction(&transaction, &inputs)?;

        self.transactions.insert(transaction, parents);

        Ok(self.chain_tip.0)
    }

    /// Returns a set of transactions for the next batch.
    ///
    /// Transactions are returned in a valid execution ordering.
    ///
    /// Returns `None` if no transactions are available.
    pub fn select_batch(&mut self) -> Option<(BatchJobId, Vec<TransactionId>)> {
        let mut parents = BTreeSet::new();
        let mut batch = Vec::with_capacity(self.batch_transaction_limit);

        for _ in 0..self.batch_transaction_limit {
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
            parents.remove(tx);
        }

        let batch_id = self.next_batch_id;
        self.next_batch_id.increment();

        self.batches.insert(batch_id, batch.clone(), parents);

        Some((batch_id, batch))
    }

    /// Drops the failed batch and all of its descendents.
    ///
    /// Transactions are placed back in the queue.
    pub fn batch_failed(&mut self, batch: BatchJobId) {
        let removed_batches = self.batches.purge_subgraphs([batch].into());

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
    pub fn batch_proved(&mut self, batch_id: BatchJobId) {
        self.batches.mark_proven(batch_id);
    }

    /// Select batches for the next block.
    ///
    /// May return an empty batch set if no batches are ready.
    ///
    /// # Panics
    ///
    /// Panics if there is already a block in flight.
    pub fn select_block(&mut self) -> (BlockNumber, BTreeSet<BatchJobId>) {
        assert!(self.block_in_progress.is_none(), "Cannot have two blocks inflight.");

        let batches = self.batches.select_block(self.block_batch_limit);
        self.block_in_progress = Some(batches.clone());

        (self.chain_tip.next(), batches)
    }

    /// Notify the pool that the block was succesfully completed.
    ///
    /// # Panics
    ///
    /// Panics if blocks are completed out-of-order or if there is no block in flight.
    pub fn block_committed(&mut self, block_number: BlockNumber) {
        assert_eq!(block_number, self.chain_tip.next(), "Blocks must be submitted sequentially");

        // Remove committed batches and transactions from graphs.
        let batches = self.block_in_progress.take().expect("No block in progress to commit");
        let transactions = self.batches.remove_committed(batches);
        let transactions = self.transactions.remove_committed(&transactions);

        // Inform inflight state about committed data.
        let impact = StateDiff::new(&transactions);
        self.state.commit_transactions(&impact);

        self.committed_blocks.push_back(impact);
        if self.committed_blocks.len() > self.staleness.0 as usize {
            // SAFETY: just checked that length is non-zero.
            let stale_block = self.committed_blocks.pop_front().unwrap();

            self.state.remove_committed_state(stale_block);
        }

        self.chain_tip.increment();
    }

    /// Block and all of its contents and dependents are purged from the mempool.
    ///
    /// # Panics
    ///
    /// Panics if there is no block in flight or if the block number does not match the current inflight block.
    pub fn block_failed(&mut self, block_number: BlockNumber) {
        assert_eq!(block_number, self.chain_tip.next(), "Blocks must be submitted sequentially");

        let batches = self.block_in_progress.take().expect("No block in progress to be failed");

        // Remove all transactions from the graphs.
        let purged = self.batches.purge_subgraphs(batches);
        let batches = purged.iter().map(|(b, _)| *b).collect::<Vec<_>>();
        let transactions = purged.into_iter().flat_map(|(_, tx)| tx.into_iter()).collect();

        let transactions = self.transactions.purge_subgraphs(transactions);

        // Rollback state.
        let impact = StateDiff::new(&transactions);
        self.state.revert_transactions(impact);
        // TODO: revert nullifiers and notes.
    }

    /// The highest block height we consider stale.
    ///
    /// Returns [None] if the blockchain is so short that all blocks are considered fresh.
    fn stale_block(&self) -> Option<BlockNumber> {
        self.chain_tip.checked_sub(self.staleness)
    }
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum AddTransactionError {
    #[error("Transaction's initial account state {expected} did not match the current account state {current}.")]
    InvalidAccountState { current: Digest, expected: Digest },
    #[error("Transaction input data is stale. Required data fresher than {stale_limit} but inputs are from {input_block}.")]
    StaleInputs {
        input_block: BlockNumber,
        stale_limit: BlockNumber,
    },
    #[error("Authenticated note nullifier {0} not found.")]
    AuthenticatedNoteNotFound(Nullifier),
    #[error("Unauthenticated note {0} not found.")]
    UnauthenticatedNoteNotFound(NoteId),
    #[error("Note nullifiers already consumed: {0:?}")]
    NotesAlreadyConsumed(BTreeSet<Nullifier>),
    #[error(transparent)]
    TxInputsError(#[from] TxInputsError),
    #[error(transparent)]
    ProofVerificationFailed(#[from] TransactionVerifierError),
    #[error("Failed to deserialize transaction: {0}.")]
    DeserializationError(String),
}

/// Describes the impact that a set of transactions had on the state.
///
/// TODO: this is a terrible name.
struct StateDiff {
    /// The number of transactions that affected each account.
    account_transactions: BTreeMap<AccountId, usize>,

    /// The nullifiers that were emitted by the transactions.
    nullifiers: BTreeSet<Nullifier>,
    // TODO: input/output notes
}

impl StateDiff {
    fn new(txs: &[Arc<ProvenTransaction>]) -> Self {
        let mut account_transactions = BTreeMap::<AccountId, usize>::new();
        let mut nullifiers = BTreeSet::new();

        for tx in txs {
            *account_transactions.entry(tx.account_id()).or_default() += 1;
            nullifiers.extend(tx.get_nullifiers());
        }

        Self { account_transactions, nullifiers }
    }
}