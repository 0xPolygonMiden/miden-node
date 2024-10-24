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
        let tx_id = self.inner.roots().first()?.clone();

        self.inner.process_root(tx_id);
        let tx = self.inner.get(&tx_id).expect("Node exists").clone();
        let parents = self.inner.parents(&tx_id).expect("Node exists").clone();

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
        self.inner.requeue(transactions)
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

// TESTS
// ================================================================================================

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::test_utils::Random;

//     /// Simplified graph type which uses the transaction ID as the data value.
//     ///
//     /// Production usage will have `T: ProvenTransaction` however this is cumbersome
//     /// to generate. Since this graph doesn't actually care about the data type, we
//     /// simplify test data generation by just duplicating the ID.
//     type TestGraph = TransactionGraph<TransactionId>;

//     /// Test helpers and aliases.
//     impl TestGraph {
//         /// Alias to insert a transaction with no parents.
//         fn insert_with_no_parent(&mut self, id: TransactionId) {
//             self.insert_with_parents(id, Default::default());
//         }

//         /// Alias for inserting a transaction with parents.
//         fn insert_with_parents(&mut self, id: TransactionId, parents: BTreeSet<TransactionId>) {
//             self.insert(id, id, parents);
//         }

//         /// Alias for inserting a transaction with a single parent.
//         fn insert_with_parent(&mut self, id: TransactionId, parent: TransactionId) {
//             self.insert_with_parents(id, [parent].into());
//         }

//         /// Calls `pop_for_processing` until it returns `None`.
//         ///
//         /// This should result in a fully processed graph, barring bugs.
//         ///
//         /// Panics if the graph is not fully processed.
//         fn process_all(&mut self) -> Vec<TransactionId> {
//             let mut processed = Vec::new();
//             while let Some((id, _)) = self.pop_for_processing() {
//                 processed.push(id);
//             }

//             assert!(self.nodes.values().all(Node::is_processed));

//             processed
//         }
//     }

//     #[test]
//     fn pruned_nodes_are_nonextant() {
//         //! Checks that processed and then pruned nodes behave as if they
//         //! never existed in the graph. We test this by comparing it to
//         //! a reference graph created without these ancestor nodes.
//         let mut rng = Random::with_random_seed();

//         let ancestor_a = rng.draw_tx_id();
//         let ancestor_b = rng.draw_tx_id();

//         let child_a = rng.draw_tx_id();
//         let child_b = rng.draw_tx_id();
//         let child_both = rng.draw_tx_id();

//         let mut uut = TestGraph::default();
//         uut.insert_with_no_parent(ancestor_a);
//         uut.insert_with_no_parent(ancestor_b);
//         uut.insert_with_parent(child_a, ancestor_a);
//         uut.insert_with_parent(child_b, ancestor_b);
//         uut.insert_with_parents(child_both, [ancestor_a, ancestor_b].into());

//         uut.process(ancestor_a);
//         uut.process(ancestor_b);
//         uut.prune_processed(&[ancestor_a, ancestor_b]);

//         let mut reference = TestGraph::default();
//         reference.insert_with_no_parent(child_a);
//         reference.insert_with_no_parent(child_b);
//         reference.insert_with_no_parent(child_both);

//         assert_eq!(uut, reference);
//     }

//     #[test]
//     fn inserted_node_is_considered_for_root() {
//         //! Ensure that a fresh node who's parent is
//         //! already processed will be considered for processing.
//         let mut rng = Random::with_random_seed();
//         let parent_a = rng.draw_tx_id();
//         let parent_b = rng.draw_tx_id();
//         let child_a = rng.draw_tx_id();
//         let child_b = rng.draw_tx_id();

//         let mut uut = TestGraph::default();
//         uut.insert_with_no_parent(parent_a);
//         uut.insert_with_no_parent(parent_b);
//         uut.process(parent_a);

//         uut.insert_with_parent(child_a, parent_a);
//         uut.insert_with_parent(child_b, parent_b);

//         assert!(uut.roots.contains(&child_a));
//         assert!(!uut.roots.contains(&child_b));
//     }

//     #[test]
//     fn fifo_order_is_maintained() {
//         //! This test creates a simple queue graph, expecting that the processed items should
//         //! be emitted in the same order.
//         let mut rng = Random::with_random_seed();
//         let input = (0..10).map(|_| rng.draw_tx_id()).collect::<Vec<_>>();

//         let mut uut = TestGraph::default();
//         uut.insert_with_no_parent(input[0]);
//         for pairs in input.windows(2) {
//             let (parent, id) = (pairs[0], pairs[1]);
//             uut.insert_with_parent(id, parent);
//         }

//         let result = uut.process_all();
//         assert_eq!(result, input);
//     }

//     #[test]
//     fn requeuing_resets_graph_state() {
//         //! Requeuing transactions should cause the internal state to reset
//         //! to the same state as before these transactions were emitted
//         //! for processing.

//         let mut rng = Random::with_random_seed();

//         let ancestor_a = rng.draw_tx_id();
//         let ancestor_b = rng.draw_tx_id();
//         let parent_a = rng.draw_tx_id();
//         let parent_b = rng.draw_tx_id();
//         let child_a = rng.draw_tx_id();
//         let child_b = rng.draw_tx_id();
//         let child_c = rng.draw_tx_id();

//         let mut uut = TestGraph::default();
//         uut.insert_with_no_parent(ancestor_a);
//         uut.insert_with_no_parent(ancestor_b);
//         uut.insert_with_parent(parent_a, ancestor_a);
//         uut.insert_with_parent(parent_b, ancestor_b);
//         uut.insert_with_parents(child_a, [ancestor_a, parent_a].into());
//         uut.insert_with_parents(child_b, [parent_a, parent_b].into());
//         uut.insert_with_parent(child_c, parent_b);

//         let mut reference = uut.clone();

//         uut.process(ancestor_a);
//         uut.process(ancestor_b);
//         uut.process(parent_a);
//         uut.process(parent_b);
//         uut.process(child_c);

//         // Requeue all except ancestor a. This is a somewhat arbitrary choice.
//         // The reference graph should therefore only have ancestor a processed.
//         uut.requeue_transactions([ancestor_b, parent_a, parent_b, child_c].into());
//         reference.process(ancestor_a);

//         assert_eq!(uut, reference);
//     }

//     #[test]
//     fn nodes_are_processed_exactly_once() {
//         let mut rng = Random::with_random_seed();

//         let ancestor_a = rng.draw_tx_id();
//         let ancestor_b = rng.draw_tx_id();
//         let parent_a = rng.draw_tx_id();
//         let parent_b = rng.draw_tx_id();
//         let child_a = rng.draw_tx_id();
//         let child_b = rng.draw_tx_id();
//         let child_c = rng.draw_tx_id();

//         let mut uut = TestGraph::default();
//         uut.insert_with_no_parent(ancestor_a);
//         uut.insert_with_no_parent(ancestor_b);
//         uut.insert_with_parent(parent_a, ancestor_a);
//         uut.insert_with_parent(parent_b, ancestor_b);
//         uut.insert_with_parents(child_a, [ancestor_a, parent_a].into());
//         uut.insert_with_parents(child_b, [parent_a, parent_b].into());
//         uut.insert_with_parent(child_c, parent_b);

//         let mut result = uut.process_all();
//         result.sort();

//         let mut expected =
//             vec![ancestor_a, ancestor_b, parent_a, parent_b, child_a, child_b, child_c];
//         expected.sort();

//         assert_eq!(result, expected);
//     }

//     #[test]
//     fn processed_data_and_parent_tracking() {
//         let mut rng = Random::with_random_seed();

//         let ancestor_a = rng.draw_tx_id();
//         let ancestor_b = rng.draw_tx_id();
//         let parent_a = rng.draw_tx_id();
//         let parent_b = rng.draw_tx_id();
//         let child_a = rng.draw_tx_id();
//         let child_b = rng.draw_tx_id();
//         let child_c = rng.draw_tx_id();

//         let mut uut = TestGraph::default();
//         uut.insert_with_no_parent(ancestor_a);
//         uut.insert_with_no_parent(ancestor_b);
//         uut.insert_with_parent(parent_a, ancestor_a);
//         uut.insert_with_parent(parent_b, ancestor_b);
//         uut.insert_with_parents(child_a, [ancestor_a, parent_a].into());
//         uut.insert_with_parents(child_b, [parent_a, parent_b].into());
//         uut.insert_with_parent(child_c, parent_b);

//         let result = uut.process(ancestor_a);
//         assert_eq!(result, (ancestor_a, Default::default()));

//         let result = uut.process(ancestor_b);
//         assert_eq!(result, (ancestor_b, Default::default()));

//         let result = uut.process(parent_a);
//         assert_eq!(result, (parent_a, [ancestor_a].into()));

//         let result = uut.process(parent_b);
//         assert_eq!(result, (parent_b, [ancestor_b].into()));

//         let result = uut.process(child_a);
//         assert_eq!(result, (child_a, [ancestor_a, parent_a].into()));

//         let result = uut.process(child_b);
//         assert_eq!(result, (child_b, [parent_a, parent_b].into()));

//         let result = uut.process(child_c);
//         assert_eq!(result, (child_c, [parent_b].into()));
//     }

//     #[test]
//     fn purging_subgraph_handles_internal_nodes() {
//         //! Purging a subgraph should correctly handle nodes already deleted within that
// subgraph.         //!
//         //! This is a concern for errors as we are deleting parts of the subgraph while we are
//         //! iterating through the nodes to purge. This means its likely a node will already
//         //! have been deleted before processing it as an input.
//         //!
//         //! We can somewhat force this to occur by re-ordering the inputs relative to the actual
//         //! dependency order.

//         let mut rng = Random::with_random_seed();

//         let ancestor_a = rng.draw_tx_id();
//         let ancestor_b = rng.draw_tx_id();
//         let parent_a = rng.draw_tx_id();
//         let parent_b = rng.draw_tx_id();
//         let child_a = rng.draw_tx_id();
//         let child_b = rng.draw_tx_id();
//         let child_c = rng.draw_tx_id();

//         let mut uut = TestGraph::default();
//         uut.insert_with_no_parent(ancestor_a);
//         uut.insert_with_no_parent(ancestor_b);
//         uut.insert_with_parent(parent_a, ancestor_a);
//         uut.insert_with_parent(parent_b, ancestor_b);
//         uut.insert_with_parents(child_a, [ancestor_a, parent_a].into());
//         uut.insert_with_parents(child_b, [parent_a, parent_b].into());
//         uut.insert_with_parent(child_c, parent_b);

//         uut.purge_subgraphs(vec![child_b, parent_a]);

//         let mut reference = TestGraph::default();
//         reference.insert_with_no_parent(ancestor_a);
//         reference.insert_with_no_parent(ancestor_b);
//         reference.insert_with_parent(parent_b, ancestor_b);
//         reference.insert_with_parent(child_c, parent_b);

//         assert_eq!(uut, reference);
//     }

//     #[test]
//     fn purging_removes_all_descendents() {
//         let mut rng = Random::with_random_seed();

//         let ancestor_a = rng.draw_tx_id();
//         let ancestor_b = rng.draw_tx_id();
//         let parent_a = rng.draw_tx_id();
//         let parent_b = rng.draw_tx_id();
//         let child_a = rng.draw_tx_id();
//         let child_b = rng.draw_tx_id();
//         let child_c = rng.draw_tx_id();

//         let mut uut = TestGraph::default();
//         uut.insert_with_no_parent(ancestor_a);
//         uut.insert_with_no_parent(ancestor_b);
//         uut.insert_with_parent(parent_a, ancestor_a);
//         uut.insert_with_parent(parent_b, ancestor_b);
//         uut.insert_with_parents(child_a, [ancestor_a, parent_a].into());
//         uut.insert_with_parents(child_b, [parent_a, parent_b].into());
//         uut.insert_with_parent(child_c, parent_b);

//         uut.purge_subgraphs(vec![parent_a]);

//         let mut reference = TestGraph::default();
//         reference.insert_with_no_parent(ancestor_a);
//         reference.insert_with_no_parent(ancestor_b);
//         reference.insert_with_parent(parent_b, ancestor_b);
//         reference.insert_with_parent(child_c, parent_b);

//         assert_eq!(uut, reference);
//     }

//     #[test]
//     #[should_panic]
//     fn duplicate_insert() {
//         let mut rng = Random::with_random_seed();
//         let mut uut = TestGraph::default();

//         let id = rng.draw_tx_id();
//         uut.insert_with_no_parent(id);
//         uut.insert_with_no_parent(id);
//     }

//     #[test]
//     #[should_panic]
//     fn missing_parents_in_insert() {
//         let mut rng = Random::with_random_seed();
//         let mut uut = TestGraph::default();

//         uut.insert_with_parents(rng.draw_tx_id(), [rng.draw_tx_id()].into());
//     }

//     #[test]
//     #[should_panic]
//     fn requeueing_an_already_queued_tx() {
//         let mut rng = Random::with_random_seed();
//         let mut uut = TestGraph::default();

//         let id = rng.draw_tx_id();
//         uut.insert_with_no_parent(id);
//         uut.requeue_transactions([id].into());
//     }
// }
