use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use miden_objects::transaction::{ProvenTransaction, TransactionId};

use super::dependency_graph::{DependencyGraph, GraphError};
use crate::domain::transaction::AuthenticatedTransaction;

// TRANSACTION GRAPH
// ================================================================================================

/// Tracks the dependency graph and status of transactions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransactionGraph {
    inner: DependencyGraph<TransactionId, AuthenticatedTransaction>,
}

impl TransactionGraph {
    /// Inserts a new transaction node, with edges to the given parent nodes.
    ///
    /// # Errors
    ///
    /// Follows the error conditions of [DependencyGraph::insert].
    pub fn insert(
        &mut self,
        transaction: AuthenticatedTransaction,
        parents: BTreeSet<TransactionId>,
    ) -> Result<(), GraphError<TransactionId>> {
        self.inner.insert(transaction.id(), transaction, parents)
    }

    /// Returns the next transaction ready for processing, and its parent edges.
    ///
    /// Internally this transaction is now marked as processed and is no longer considered inqueue.
    pub fn pop_for_processing(
        &mut self,
    ) -> Option<(AuthenticatedTransaction, BTreeSet<TransactionId>)> {
        let root = self.inner.roots().first()?.clone();

        self.inner.process_root(root).expect("This is definitely a root");
        let tx = self.inner.get(&root).expect("Node exists").clone();
        let parents = self.inner.parents(&root).expect("Node exists").clone();

        Some((tx, parents))
    }

    /// Marks the given transactions as being back in queue.
    ///
    /// # Errors
    ///
    /// Follows the error conditions of [DependencyGraph::requeue].
    pub fn requeue_transactions(
        &mut self,
        transactions: BTreeSet<TransactionId>,
    ) -> Result<(), GraphError<TransactionId>> {
        self.inner.revert_subgraphs(transactions)
    }

    /// Removes committed transactions, pruning them from the graph.
    ///
    /// # Errors
    ///
    /// Follows the error conditions of [DependencyGraph::prune_processed].
    pub fn remove_committed(
        &mut self,
        tx_ids: &[TransactionId],
    ) -> Result<Vec<AuthenticatedTransaction>, GraphError<TransactionId>> {
        // TODO: revisit this api.
        let tx_ids = tx_ids.into_iter().cloned().collect();
        self.inner.prune_processed(tx_ids)
    }

    /// Removes the transactions and all their descendants from the graph.
    ///
    /// Returns the removed transactions.
    ///
    /// # Errors
    ///
    /// Follows the error conditions of [DependencyGraph::purge_subgraphs].
    pub fn purge_subgraphs(
        &mut self,
        transactions: Vec<TransactionId>,
    ) -> Result<Vec<AuthenticatedTransaction>, GraphError<TransactionId>> {
        // TODO: revisit this api.
        let transactions = transactions.into_iter().collect();
        self.inner.purge_subgraphs(transactions)
    }
}
