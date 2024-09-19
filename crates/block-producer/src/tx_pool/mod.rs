#![allow(unused)]

use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
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
}

pub struct TransactionPool {
    /// The latest inflight state of each account.
    ///
    /// Accounts without inflight transactions are not stored.
    account_state: BTreeMap<AccountId, (Digest, TransactionId)>,

    /// Block number at which transaction inputs are considered stale.
    ///
    /// This means we track inflight data and in blocks completed after this block,
    /// **excluding** this block.
    stale_block: BlockNumber,

    /// The number of the next block we hand out.
    next_block: BlockNumber,

    /// All transactions currently inflight.
    ///
    /// This includes those not yet processed, those in batches and blocks after the stale block.
    tx_pool: BTreeMap<TransactionId, InflightTransaction>,

    /// Transactions ready to be included in a batch.
    ///
    /// aka transactions whose parent transactions are already included in batches.
    tx_roots: BTreeSet<TransactionId>,

    /// The next batches ID.
    next_batch_id: BatchId,

    batch_pool: BTreeMap<BatchId, InflightBatch>,

    /// Batches which are ready to be included in a block.
    ///
    /// aka batches who's parent batches are already included in blocks.
    batch_roots: BTreeSet<BatchId>,

    /// Blocks which are inflight or completed but not yet considered stale.
    block_pool: BTreeMap<BlockNumber, Vec<BatchId>>,

    /// The next block we expect to complete.
    next_completed_block: BlockNumber,
}

impl TransactionPool {
    /// Complete barring todos.
    pub fn add_transaction(
        mut self,
        transaction: ProvenTransaction,
        mut inputs: TransactionInputs,
    ) -> Result<(), AddTransactionError> {
        // Ensure inputs aren't stale.
        if inputs.current_block_height <= self.stale_block.0 {
            return Err(AddTransactionError::StaleInputs {
                input_block: BlockNumber(inputs.current_block_height),
                stale_limit: self.stale_block,
            });
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

        // Inform parent's of their new child.
        for parent in &parents {
            self.tx_pool.get_mut(parent).expect("Parent must be in pool").add_child(tx_id);
        }

        // Insert transaction into pool and possibly as a root transaction.
        self.tx_pool.insert(tx_id, InflightTransaction::new(transaction, parents));
        self.try_root_transaction(tx_id);

        Ok(())
    }

    /// Returns at most `count` transactions and a batch ID.
    ///
    /// Transactions are returned in a valid execution ordering.
    ///
    /// Returns `None` if no transactions are available.
    pub fn select_batch(&mut self, count: usize) -> Option<(BatchId, Vec<Arc<ProvenTransaction>>)> {
        if self.tx_roots.is_empty() {
            tracing::debug!("No transactions available for requested batch");
            return None;
        }

        // Ideally we would use a hash over transaction ID here but that would be expensive.
        let batch_id = self.next_batch_id;
        self.next_batch_id.increment();

        let mut parent_batches = BTreeSet::new();

        let mut batch = Vec::with_capacity(count);
        for _ in 0..count {
            // Select transactions according to some strategy here. For now its just arbitrary.
            let Some(tx) = self.tx_roots.pop_first() else {
                break;
            };
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

        // Update local book keeping, informing parent batches of their new child.
        let tx_indices = batch.iter().map(|tx| tx.id()).collect();
        for parent in &parent_batches {
            self.batch_pool
                .get_mut(parent)
                .expect("Parent batch must be in pool")
                .add_child(batch_id);
        }

        let local_batch = InflightBatch::new(tx_indices, parent_batches);
        self.batch_pool.insert(batch_id, local_batch);

        Some((batch_id, batch))
    }

    /// Drops the failed batch and all of its descendents.
    ///
    /// Transactions are placed back in the queue.
    pub fn batch_failed(&mut self, batch: BatchId) {
        // Skip non-existent batches. Its possible for this batch to have been
        // removed as a descendent of a previously failed batch.
        if !self.batch_pool.contains_key(&batch) {
            tracing::debug!(%batch, "Ignoring unknwon failed batch");
            return;
        }

        let (batches, mut transactions) = self.batch_descendents(batch);

        // Drop all impacted batches and inform parent batches that they've lost these children.
        //
        // We could also re-attempt the batch but we don't have
        // the information yet to make such a call. This could also be grounds for a complete
        // shutdown instead.
        for batch_id in &batches {
            let batch = self.batch_pool.remove(batch_id).expect("Batch must exist in pool");

            for parent in batch.parents {
                // Its possible for a parent to be removed as part of this set already.
                if let Some(parent) = self.batch_pool.get_mut(&parent) {
                    parent.remove_child(batch_id);
                }
            }
        }

        // Mark all transactions as back in-queue.
        for tx in &transactions {
            self.tx_pool.get_mut(tx).expect("Transaction must be in pool").status =
                TransactionStatus::InQueue;
        }

        let impacted_transactions = transactions.len();

        // Check all transactions as possible roots. We also need to recheck the current roots as
        // they may now be invalidated as roots.
        transactions.extend(self.tx_roots.clone());
        self.tx_roots.clear();
        for tx in transactions {
            self.try_root_transaction(tx);
        }

        tracing::warn!(%batch, descendents=?batches, %impacted_transactions, "Batch failed, dropping all inflight descendent batches, impacted transactions are back in queue.");
    }

    pub fn batch_complete(&mut self, batch_id: BatchId) {
        let Some(batch) = self.batch_pool.get_mut(&batch_id) else {
            tracing::warn!(%batch_id, "Ignoring unknown completed batch.");
            return;
        };

        batch.status = BatchStatus::Proven;

        self.try_root_batch(batch_id);
    }

    /// Select at most `count` batches which are ready to be placed into the next block.
    ///
    /// May return an empty batch set if no batches are ready.
    pub fn select_block(&mut self, count: usize) -> (BlockNumber, Vec<BatchId>) {
        // TODO: should return actual batch transaction data as well.

        let mut batches = Vec::with_capacity(count);
        let block_number = self.next_block;
        self.next_block.increment();

        // Select batches according to some strategy. Currently this is simply arbitrary.
        for _ in 0..count {
            let Some(batch) = self.batch_roots.pop_first() else {
                break;
            };
            batches.push(batch);

            // Update status and check child batches for rootability.
            let batch = self.batch_pool.get_mut(&batch).expect("Batch must be in pool");
            batch.status = BatchStatus::Blocked;

            // Note: this does not handle batches with circular dependencies.
            //
            // The current model does not create them as batches are handed out sequentially.
            // i.e. dependencies only go in one direction.
            for child in batch.children.clone() {
                self.try_root_batch(child);
            }

            // Unlike `select_batch` we don't need to track block depedencies. This is because
            // block's have an inherit sequential dependency.
        }

        assert!(batches.len() <= count, "Must return at most `count` batches");

        (block_number, batches)
    }

    /// Notify the pool that the block was succesfully completed.
    ///
    /// Panics if blocks are completed out-of-order. todo: might be a better way, but this is pretty
    /// unrecoverable..
    pub fn block_completed(&mut self, block_number: BlockNumber) {
        assert_eq!(
            block_number, self.next_completed_block,
            "Blocks must be submitted sequentially"
        );

        // Update book keeping by removing the inflight data that just became stale.
        self.stale_block.increment();
        self.next_completed_block.increment();

        let Some(stale_batches) = self.block_pool.remove(&self.stale_block) else {
            // We expect no stale blocks at startup. Alternatively we could improve the stale block
            // tracing to account for this instead.
            return;
        };

        // Update batch and transaction dependencies to forget about all batches and transactions in
        // this block.
        for batch_id in stale_batches {
            let batch = self.batch_pool.remove(&batch_id).expect("Batch must be in pool");

            for child in batch.children {
                // Its possible for a child to already be removed as part of this set of stale
                // batches.
                if let Some(child) = self.batch_pool.get_mut(&child) {
                    child.remove_parent(&batch_id);
                }
            }

            for tx_id in batch.transactions {
                let tx = self.tx_pool.remove(&tx_id).expect("Transaction must be in pool");

                // Remove mentions from inflight state.
                //
                // Its possible for the state to already have been removed by another stale
                // transaction. TODO: notes and nullifiers.
                if let Entry::Occupied(account) = self.account_state.entry(tx.data.account_id()) {
                    if account.get().1 == tx_id {
                        account.remove();
                    }
                }

                for child in tx.children {
                    // Its possible for a child to already be removed as part of this set of stale
                    // batches.
                    if let Some(child) = self.tx_pool.get_mut(&child) {
                        child.remove_parent(&tx_id);
                    }
                }
            }
        }
    }

    pub fn block_failed(&mut self, block: BlockNumber) {
        // TBD.. not quite sure what to do here yet. Presumably the caller has already retried this
        // block so the block is just inherently broken.
        //
        // Given lack of information at this stage we should probably just abort the node?
        // In the future we might improve the situation with more fine-grained failure reasons.
    }

    /// Returns the batch, its descendents and all their transactions.
    fn batch_descendents(&self, batch: BatchId) -> (BTreeSet<BatchId>, BTreeSet<TransactionId>) {
        // Iterative implementation to prevent stack overflow and issues with multiple parents.
        let mut to_process = vec![batch];
        let mut descendents = BTreeSet::new();
        let mut transactions = BTreeSet::new();

        while let Some(batch) = to_process.pop() {
            // Guard against repeat processing. This is possible because a batch can have multiple
            // parents.
            if descendents.insert(batch) {
                let batch = self.batch_pool.get(&batch).expect("Batch should exist");

                transactions.extend(&batch.transactions);
                to_process.extend(&batch.children);
            }
        }

        (descendents, transactions)
    }

    /// Adds the transaction to the set of roots IFF all of its parent's have been processed.
    fn try_root_transaction(&mut self, tx_id: TransactionId) {
        let tx = self.tx_pool.get(&tx_id).expect("Transaction mut be in pool");
        for parent in &tx.parents {
            let parent = self.tx_pool.get(parent).expect("Parent must be in pool");

            if parent.status == TransactionStatus::InQueue {
                return;
            }
        }

        self.tx_roots.insert(tx_id);
    }

    /// Add the batch as a root if all parents have been placed into blocks.
    fn try_root_batch(&mut self, batch_id: BatchId) {
        let batch = self.batch_pool.get_mut(&batch_id).expect("Batch must be in pool");
        for parent in &batch.parents.clone() {
            let parent = self.batch_pool.get(parent).expect("Parent batch must be in pool");

            if parent.status != BatchStatus::Blocked {
                return;
            }
        }
        self.batch_roots.insert(batch_id);
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

struct InflightTransaction {
    status: TransactionStatus,
    data: Arc<ProvenTransaction>,
    parents: BTreeSet<TransactionId>,
    children: BTreeSet<TransactionId>,
}

impl InflightTransaction {
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

    fn remove_parent(&mut self, parent: &TransactionId) {
        self.parents.remove(parent);
    }
}

struct InflightBatch {
    status: BatchStatus,
    transactions: Vec<TransactionId>,
    parents: BTreeSet<BatchId>,
    children: BTreeSet<BatchId>,
}

impl InflightBatch {
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

    fn remove_child(&mut self, child: &BatchId) {
        self.children.remove(child);
    }

    fn remove_parent(&mut self, parent: &BatchId) {
        self.parents.remove(parent);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionStatus {
    InQueue,
    /// Part of an inflight batch.
    Batched(BatchId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchStatus {
    /// Dispatched for proving.
    Inflight,
    /// Proven.
    Proven,
    /// Part of an inflight block.
    Blocked,
}
