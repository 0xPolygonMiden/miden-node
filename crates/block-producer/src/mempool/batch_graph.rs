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

#[derive(Default, Clone)]
pub struct BatchGraph {
    inner: DependencyGraph<BatchJobId, TransactionBatch>,

    /// Allows for reverse lookup of transaction -> batch.
    transactions: BTreeMap<TransactionId, BatchJobId>,

    batches: BTreeMap<BatchJobId, Vec<TransactionId>>,
}

impl BatchGraph {
    pub fn insert(
        &mut self,
        id: BatchJobId,
        transactions: Vec<TransactionId>,
        parents: BTreeSet<TransactionId>,
    ) -> Result<(), GraphError<BatchJobId>> {
        // Reverse lookup parent transaction batches.
        let parents = parents
            .into_iter()
            .map(|tx| self.transactions.get(&tx).expect("Parent transaction must be in a batch"))
            .copied()
            .collect();

        self.inner.insert_pending(id, parents)?;

        for tx in &transactions {
            self.transactions.insert(tx.clone(), id);
        }
        self.batches.insert(id, transactions);

        Ok(())
    }

    /// Removes the batches and all their descendants from the graph.
    ///
    /// Returns all removed batches and their transactions.
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

    /// Removes a set of batches from the graph without removing any descendants.
    ///
    /// This is intended to cull completed batches from stale blocs.
    pub fn remove_committed(
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

    /// Mark a batch as proven if it exists.
    pub fn mark_proven(
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
            let Some(batch_id) = self.inner.roots().first().copied() else {
                break;
            };

            self.inner.process_root(batch_id).expect("This is a root");
            batches.insert(
                batch_id,
                self.inner.get(&batch_id).expect("Root batch must have a value").clone(),
            );
        }

        batches
    }
}
