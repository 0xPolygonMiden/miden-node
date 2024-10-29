use std::collections::{BTreeMap, BTreeSet};

use miden_objects::transaction::TransactionId;
use miden_tx::utils::collections::KvMap;

use super::{
    dependency_graph::{DependencyGraph, GraphError},
    BatchJobId,
};
use crate::batch_builder::batch::TransactionBatch;

// BATCH GRAPH
// ================================================================================================

/// Tracks the dependencies between batches, transactions and their parents.
///
/// Batches are inserted with their transaction, and parent transaction sets which form the edges of
/// the dependency graph. Batches are initially inserted in a pending state while we wait on their
/// proofs to be generated. The dependencies are still tracked in this state.
///
/// Batches can then be promoted to ready by [submitting their proofs](Self::submit_proof) once
/// available. Proven batches are considered for inclusion in blocks once _all_ parent batches have
/// been selected.
///
/// Committed batches (i.e. included in blocks) may be [pruned](Self::prune_committed) from the
/// graph to bound the graph's size.
///
/// Batches may also be outright [purged](Self::purge_subgraphs) from the graph. This is useful for
/// batches which may have become invalid due to external considerations e.g. expired transactions.
#[derive(Default, Clone)]
pub struct BatchGraph {
    /// Tracks the interdependencies between batches.
    inner: DependencyGraph<BatchJobId, TransactionBatch>,

    /// Maps each transaction to its batch, allowing for reverse lookups.
    ///
    /// Incoming batches are defined entirely in terms of transactions, including parent edges.
    /// This let's us transform these parent transactions into the relevant parent batches.
    transactions: BTreeMap<TransactionId, BatchJobId>,

    /// Maps each batch to its transaction set.
    ///
    /// Required because the dependency graph is defined in terms of batches. This let's us
    /// translate between batches and their transactions when required.
    batches: BTreeMap<BatchJobId, Vec<TransactionId>>,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum BatchInsertError {
    #[error("Transactions are already in the graph: {0:?}")]
    DuplicateTransactions(BTreeSet<TransactionId>),
    #[error("Unknown parent transaction {0}")]
    UknownParentTransaction(TransactionId),
    #[error(transparent)]
    GraphError(#[from] GraphError<BatchJobId>),
}

impl BatchGraph {
    /// Inserts a new batch into the graph.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///   - the batch ID is already in use
    ///   - any transactions are already in the graph
    ///   - any parent transactions are _not_ in the graph
    pub fn insert(
        &mut self,
        id: BatchJobId,
        transactions: Vec<TransactionId>,
        parents: BTreeSet<TransactionId>,
    ) -> Result<(), BatchInsertError> {
        let duplicates = transactions
            .iter()
            .filter(|tx| self.transactions.contains_key(tx))
            .copied()
            .collect::<BTreeSet<_>>();
        if !duplicates.is_empty() {
            return Err(BatchInsertError::DuplicateTransactions(duplicates));
        }

        // Reverse lookup parent transaction batches.
        let parent_batches = parents
            .into_iter()
            .map(|tx| {
                self.transactions
                    .get(&tx)
                    .copied()
                    .ok_or(BatchInsertError::UknownParentTransaction(tx))
            })
            .collect::<Result<_, _>>()?;

        self.inner.insert_pending(id, parent_batches)?;

        for tx in transactions.iter().copied() {
            self.transactions.insert(tx, id);
        }
        self.batches.insert(id, transactions);

        Ok(())
    }

    /// Removes the batches and their descendants from the graph.
    ///
    /// Returns all removed batches and their transactions.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the batches are not currently in the graph.
    pub fn purge_subgraphs(
        &mut self,
        batches: BTreeSet<BatchJobId>,
    ) -> Result<BTreeMap<BatchJobId, Vec<TransactionId>>, GraphError<BatchJobId>> {
        let batches = self.inner.purge_subgraphs(batches)?;

        let batches = batches
            .into_iter()
            .map(|batch| (batch, self.batches.remove(&batch).expect("Malformed graph")))
            .collect::<BTreeMap<_, _>>();

        for tx in batches.values().flatten() {
            self.transactions.remove(tx);
        }

        Ok(batches)
    }

    /// Removes set set of committed batches from the graph.
    ///
    /// The batches _must_ have been previously selected for inclusion in a block using
    /// [`select_block`](Self::select_block). This is intended for limiting the size of the graph by
    /// culling committed data.
    ///
    /// # Errors
    ///
    /// Returns an error if
    ///   - any batch was not previously selected for inclusion in a block
    ///   - any batch is unknown
    ///   - any parent batch would be left dangling in the graph
    ///
    /// The last point implies that batches should be removed in block order.
    pub fn prune_committed(
        &mut self,
        batches: BTreeSet<BatchJobId>,
    ) -> Result<Vec<TransactionId>, GraphError<BatchJobId>> {
        self.inner.prune_processed(batches.clone())?;
        let mut transactions = Vec::new();

        for batch in &batches {
            transactions.extend(self.batches.remove(batch).into_iter().flatten());
        }

        for tx in &transactions {
            self.transactions.remove(tx);
        }

        Ok(transactions)
    }

    /// Submits a proof for the given batch, promoting it from pending to ready for inclusion in a
    /// block once all its parents have themselves been included.
    ///
    /// # Errors
    ///
    /// Returns an error if the batch is not in the graph or if it was already previously proven.
    pub fn submit_proof(
        &mut self,
        id: BatchJobId,
        batch: TransactionBatch,
    ) -> Result<(), GraphError<BatchJobId>> {
        self.inner.promote_pending(id, batch)
    }

    /// Returns at most `count` batches which are ready for inclusion in a block.
    pub fn select_block(&mut self, count: usize) -> BTreeMap<BatchJobId, TransactionBatch> {
        let mut batches = BTreeMap::new();

        for _ in 0..count {
            // This strategy just selects arbitrary roots for now. This is valid but not very
            // interesting or efficient.
            let Some(batch_id) = self.inner.roots().first().copied() else {
                break;
            };

            // SAFETY: This is definitely a root since we just selected it from the set of roots.
            self.inner.process_root(batch_id).unwrap();
            // SAFETY: Since it was a root batch, it must definitely have a processed batch
            // associated with it.
            let batch = self.inner.get(&batch_id).unwrap();

            batches.insert(batch_id, batch.clone());
        }

        batches
    }
}
