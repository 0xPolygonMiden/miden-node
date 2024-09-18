#![allow(unused)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    sync::Arc,
};

use miden_objects::{
    accounts::AccountId,
    transaction::{ProvenTransaction, TransactionId},
    Digest,
};

use crate::store::TransactionInputs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BatchId(u64);

impl Display for BatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockNumber(u32);

impl Display for BlockNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub struct TransactionPool {
    account_state: BTreeMap<AccountId, (Digest, TransactionId)>,

    /// Block number at which transaction inputs are considered stale.
    stale_block: BlockNumber,

    /// All transactions currently inflight. This includes those not yet processed, those in batches and those in an inflight block.
    tx_pool: BTreeMap<TransactionId, Transaction>,

    /// Set of transactions who's depedencies have all been processed already.
    ///
    /// In other words, transactions which are available to process next.
    tx_roots: BTreeSet<TransactionId>,

    /// The next batches ID.
    next_batch_id: u64,

    batch_pool: BTreeMap<BatchId, Batch>,
    batch_roots: BTreeSet<BatchId>,
}

impl TransactionPool {
    /// Returns a batch of transactions and a batch ID.
    pub fn select_batch(&mut self) -> Option<(BatchId, Vec<Arc<ProvenTransaction>>)> {
        if self.tx_roots.is_empty() {
            tracing::debug!("No transactions available for requested batch");
            return None;
        }

        // Ideally we would use a hash over transaction ID here but that would be expensive.
        let batch_id = BatchId(self.next_batch_id);
        self.next_batch_id += 1;

        let mut parent_batches = BTreeSet::new();

        // Select transactions according to some strategy here. For now its just arbitrary.
        let mut batch = Vec::new();
        while let Some(tx) = self.tx_roots.pop_first() {
            let tx = self.tx_pool.get_mut(&tx).expect("Transaction must be in pool");
            tx.status = TransactionStatus::Batched(batch_id);
            batch.push(Arc::clone(&tx.data));

            // Work around multiple borrows of self.
            let parents = tx.parents.clone();
            let children = tx.children.clone();

            // Check if any of the child transactions are now rootable.
            for child in children {
                self.try_root_transaction(child);
            }

            // Track batch dependencies.
            for parent in parents {
                if let Some(parent_batch) = self
                    .tx_pool
                    .get(&parent)
                    .expect("Parent transaction must be in pool")
                    .batch_id()
                {
                    // Exclude the current batch ID -- this would be self-referencial otherwise.
                    if parent_batch != batch_id {
                        parent_batches.insert(parent_batch);
                    }
                }
            }
        }

        // Update local book keeping.
        let tx_indices = batch.iter().map(|tx| tx.id()).collect();
        for parent in &parent_batches {
            self.batch_pool
                .get_mut(&parent)
                .expect("Parent batch must be in pool")
                .add_child(batch_id);
        }
        let local_batch = Batch::new(tx_indices, parent_batches);
        self.batch_pool.insert(batch_id, local_batch);

        Some((batch_id, batch))
    }

    /// Drops the failed batch and all of its descendents.
    ///
    /// Transactions are placed back in the queue.
    ///
    /// Complete afaik.
    pub fn batch_failed(&mut self, batch: BatchId) {
        // Skip non-existent batches. Its possible for this batch to have been
        // removed as a descendent of a previously failed batch.
        if !self.batch_pool.contains_key(&batch) {
            tracing::debug!(%batch, "Ignoring unknwon failed batch");
            return;
        }

        let (batches, mut transactions) = self.batch_descendents(batch);

        // Drop all impacted batches.
        //
        // We could also re-attempt the batch but right now we don't have
        // the information yet to make such a call.
        for batch in &batches {
            self.batch_pool.remove(batch);
        }

        // Mark all transactions as back in-queue.
        for tx in &transactions {
            self.tx_pool.get_mut(tx).expect("Transaction must be in pool").status =
                TransactionStatus::InQueue;
        }

        let impacted_transactions = transactions.len();

        // Check all transactions as possible roots. We also need to recheck the current roots as
        // they may have become invalid roots.
        transactions.extend(self.tx_roots.clone().into_iter());
        self.tx_roots.clear();
        for tx in transactions {
            self.try_root_transaction(tx);
        }

        tracing::warn!(%batch, descendents=?batches, %impacted_transactions, "Batch failed, dropping all inflight descendent batches, impacted transactions are back in queue.");
    }

    /// Returns the descendents of a batch that are in the pool i.e. children, grandchildren, etc.
    /// including their transactions.
    ///
    /// The input batch is included.
    ///
    /// Complete afaik.
    fn batch_descendents(&self, batch: BatchId) -> (BTreeSet<BatchId>, BTreeSet<TransactionId>) {
        let mut to_process = vec![batch];
        let mut descendents = BTreeSet::new();
        let mut transactions = BTreeSet::new();

        while let Some(batch) = to_process.pop() {
            // Guard against repeat processing. This is possible because a batch can have multiple parents.
            if descendents.insert(batch) {
                let batch = self.batch_pool.get(&batch).expect("Batch should exist");

                transactions.extend(&batch.transactions);
                to_process.extend(&batch.children);
            }
        }

        (descendents, transactions)
    }

    /// Adds a transaction to the set of roots IFF all of its parent's are no longer in-queue.
    ///
    /// Naming is hard. What should this be called?
    ///
    /// Complete.
    fn try_root_transaction(&mut self, tx_id: TransactionId) {
        if let Some(tx) = self.tx_pool.get(&tx_id) {
            for parent in &tx.parents {
                let parent = self.tx_pool.get(&parent).expect("Parent must be in pool still");

                if parent.status == TransactionStatus::InQueue {
                    return;
                }
            }

            // All parents are already processed (in a batch, block or already stored), and we can therefore add this transaction as a root.
            self.tx_roots.insert(tx_id);
        }
    }

    pub fn batch_complete(&mut self, batch_id: BatchId) {
        let Some(batch) = self.batch_pool.get_mut(&batch_id) else {
            tracing::warn!(%batch_id, "Ignoring unknown completed batch.");
            return;
        };

        batch.status = BatchStatus::Complete;

        // Add the batch as a root if all parents are complete. No wait this is wrong -- update me tomorrow!!!!
        // Should only be a root if all parents are inflight in a block.
        for parent in &batch.parents.clone() {
            let parent = self.batch_pool.get(parent).expect("Parent batch must be in pool");

            if parent.status != BatchStatus::Complete {
                return;
            }
        }
        self.batch_roots.insert(batch_id);
    }

    /// Complete barring todos.
    pub fn add_transaction(
        mut self,
        transaction: ProvenTransaction,
        mut inputs: TransactionInputs,
    ) -> Result<(), AddTransactionError> {
        if inputs.current_block_height <= self.stale_block.0 {
            return Err(AddTransactionError::StaleInputs {
                input_block: BlockNumber(inputs.current_block_height),
                stale_limit: self.stale_block,
            });
        }

        let account_update = transaction.account_update();
        let mut parents = BTreeSet::new();

        // Merge inflight state with inputs.
        //
        // This gives us the latest applicable state for this transaction.
        // TODO: notes and nullifiers.
        if let Some((state, parent)) = self.account_state.get(&account_update.account_id()) {
            parents.insert(*parent);
            inputs.account_hash = Some(*state);
        }

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

        // Update parents to point back to this new transaction.
        for parent in &parents {
            // State information is currently not updated as transactions get removed from the pool.
            // So its possible to not have the parent exist right now.
            //
            // TODO: consider whether we can expect this instead.
            if let Some(parent) = self.tx_pool.get_mut(parent) {
                parent.add_child(tx_id);
            }
        }

        // Insert transaction into pool and possibly as a root transaction.
        self.tx_pool.insert(tx_id, Transaction::new(transaction, parents));
        self.try_root_transaction(tx_id);

        Ok(())
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

struct Transaction {
    status: TransactionStatus,
    data: Arc<ProvenTransaction>,
    parents: BTreeSet<TransactionId>,
    children: BTreeSet<TransactionId>,
}

impl Transaction {
    /// Creates a new in-queue transaction with no children.
    fn new(data: ProvenTransaction, parents: BTreeSet<TransactionId>) -> Self {
        Self {
            data: Arc::new(data),
            status: TransactionStatus::InQueue,
            parents,
            children: Default::default(),
        }
    }

    fn add_child(&mut self, child: TransactionId) {
        self.children.insert(child);
    }

    fn batch_id(&self) -> Option<BatchId> {
        match self.status {
            TransactionStatus::Batched(id) => Some(id),
            _ => None,
        }
    }
}

struct Batch {
    status: BatchStatus,
    transactions: Vec<TransactionId>,
    parents: BTreeSet<BatchId>,
    children: BTreeSet<BatchId>,
}

impl Batch {
    fn new(transactions: Vec<TransactionId>, parents: BTreeSet<BatchId>) -> Self {
        Self {
            status: BatchStatus::Inflight,
            transactions,
            parents,
            children: Default::default(),
        }
    }

    fn add_child(&mut self, child: BatchId) {
        self.children.insert(child);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionStatus {
    InQueue,
    Batched(BatchId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchStatus {
    Inflight,
    Complete,
}
