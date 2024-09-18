use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use miden_objects::{
    accounts::AccountId,
    transaction::{ProvenTransaction, TransactionId},
    Digest,
};

use crate::store::TransactionInputs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct BatchId(u64);

pub struct TransactionPool {
    account_state: BTreeMap<AccountId, Digest>,

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
                self.check_rootable(child);
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

    pub fn batch_failed(&mut self, batch: BatchId) {
        let mut to_remove = vec![batch];
        let mut txs = Vec::new();

        while let Some(batch_id) = to_remove.pop() {
            if let Some((children, batch_txs)) = self.inflight_batches.remove(&batch_id) {
                to_remove.extend(children);
                txs.extend(batch_txs.into_iter());
            } else {
                tracing::debug!(?batch_id, "Ignoring batch as it is not an inflight batch.")
            }
        }

        // Mark all transactions as back in-queue.
        for tx in &txs {
            self.tx_pool.get_mut(tx).expect("Transaction must be in pool").status =
                TransactionStatus::InQueue;
        }

        // Check all transactions as possible roots. We also need to recheck the current roots as
        // they may have become invalid roots.
        txs.extend(self.tx_roots.clone().into_iter());
        self.tx_roots.clear();
        for tx in txs {
            self.check_rootable(tx);
        }
    }

    /// Adds a transaction to the set of roots IFF all of its parent's are no longer in-queue.
    pub fn check_rootable(&mut self, tx_id: TransactionId) {
        if let Some(tx) = self.tx_pool.get(&tx_id) {
            for parent in &tx.parents {
                let parent_status =
                    self.tx_pool.get(&parent).expect("Parent must be in pool still").status;

                if parent_status == TransactionStatus::InQueue {
                    return;
                }
            }

            // All parents are already processed (in a batch, block or already stored), and we can therefore add this transaction as a root.
            self.tx_roots.insert(tx_id);
        }
    }

    pub fn batch_complete(&mut self, batch: BatchId) {
        // TODO: the rest of the owl.
        if 
    }

    pub fn add_transaction(
        mut self,
        transaction: ProvenTransaction,
        inputs: TransactionInputs,
    ) -> Result<(), AddTransactionError> {
        // TODO: verify that transaction inputs are not stale.

        // Validate inflight state.
        let account_update = transaction.account_update();
        // The current account state hash.
        let account_state = self
            .account_state
            .get(&account_update.account_id())
            .cloned()
            .or(inputs.account_hash)
            .unwrap_or_default();

        if account_state != account_update.init_state_hash() {
            return Err(AddTransactionError::InvalidAccountState {
                current: account_state,
                expected: account_update.init_state_hash(),
            });
        }

        // TODO: notes and nullifiers.

        // Transaction is valid, update inflight state.
        self.account_state
            .insert(transaction.account_id(), account_update.final_state_hash());
        // TODO: update notes and nullifiers.

        // Insert transaction into pool and possibly as a root transaction.
        let tx_id = transaction.id();
        self.tx_pool.insert(tx_id, Transaction::new(transaction));
        self.check_rootable(tx_id);

        // TODO: update depedency graph aka add parents, and update parents.

        Ok(())
    }
}

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum AddTransactionError {
    #[error("Transaction's initial account state {expected} did not match the current account state {current}.")]
    InvalidAccountState { current: Digest, expected: Digest },
}

struct Transaction {
    status: TransactionStatus,
    data: Arc<ProvenTransaction>,
    parents: BTreeSet<TransactionId>,
    children: BTreeSet<TransactionId>,
}

impl Transaction {
    /// Creates a new in-queue transaction with no children and no parents.
    fn new(data: ProvenTransaction) -> Self {
        Self {
            data: Arc::new(data),
            status: TransactionStatus::InQueue,
            parents: Default::default(),
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
