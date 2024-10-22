use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use miden_objects::transaction::{ProvenTransaction, TransactionId};

// TRANSACTION GRAPH
// ================================================================================================

/// Tracks the dependency graph and status of transactions.
#[derive(Clone, Debug, PartialEq)]
pub struct TransactionGraph<T> {
    /// All transactions currently inflight.
    nodes: BTreeMap<TransactionId, Node<T>>,

    /// Transactions ready for being processed.
    ///
    /// aka transactions whose parents are already processed.
    roots: BTreeSet<TransactionId>,
}

impl<T> Default for TransactionGraph<T> {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            roots: Default::default(),
        }
    }
}

impl<T: Clone> TransactionGraph<T> {
    /// Inserts a new transaction node, with edges to the given parent nodes.
    ///
    /// # Panics
    ///
    /// Panics if:
    ///    - any of the given parents are not part of the graph,
    ///    - the transaction is already present
    pub fn insert(&mut self, id: TransactionId, data: T, parents: BTreeSet<TransactionId>) {
        // Inform parents of their new child.
        for parent in &parents {
            self.nodes.get_mut(parent).expect("Parent must be in pool").children.insert(id);
        }

        let node = Node::new(data, parents);
        if self.nodes.insert(id, node).is_some() {
            panic!("Transaction already exists in pool");
        }

        // This could be optimized by inlining this inside the parent loop. This would prevent the
        // double iteration over parents, at the cost of some code duplication.
        self.try_make_root(id);
    }

    /// Returns the next transaction ready for processing, and its parent edges.
    ///
    /// Internally this transaction is now marked as processed and is no longer considered inqueue.
    pub fn pop_for_processing(&mut self) -> Option<(T, BTreeSet<TransactionId>)> {
        let tx_id = self.roots.first()?;

        Some(self.process(*tx_id))
    }

    /// Marks the transaction as processed and returns its data and parents.
    ///
    /// Separated out from the actual strategy of choosing so that we have more
    /// fine grained control available for tests.
    ///
    /// # Panics
    ///
    /// Panics if the transaction:
    ///   - does not exist
    ///   - is already processed
    ///   - is not ready for processing
    fn process(&mut self, id: TransactionId) -> (T, BTreeSet<TransactionId>) {
        assert!(self.roots.remove(&id), "Process target must form part of roots");
        let node = self.nodes.get_mut(&id).expect("Root transaction must be in graph");
        node.mark_as_processed();

        // Work around multiple mutable borrows of self.
        let parents = node.parents.clone();
        let children = node.children.clone();
        let tx = node.data.clone();

        for child in children {
            self.try_make_root(child);
        }

        (tx, parents)
    }

    /// Marks the given transactions as being back in queue.
    ///
    /// # Panics
    ///
    /// Panics if any of the transactions are
    /// - not part of the graph
    /// - are already in queue aka not processed
    pub fn requeue_transactions(&mut self, transactions: BTreeSet<TransactionId>) {
        for tx in &transactions {
            self.nodes.get_mut(tx).expect("Node must exist").mark_as_inqueue();
        }

        // All requeued transactions are potential roots, and current roots may have been
        // invalidated.
        let mut potential_roots = std::mem::take(&mut self.roots);
        potential_roots.extend(transactions);
        for tx in potential_roots {
            self.try_make_root(tx);
        }
    }

    /// Prunes processed transactions from the graph.
    ///
    /// # Panics
    ///
    /// Panics if any of the given transactions are:
    ///   - not part of the graph
    ///   - are in queue aka not processed
    pub fn prune_processed(&mut self, tx_ids: &[TransactionId]) -> Vec<T> {
        let mut transactions = Vec::with_capacity(tx_ids.len());
        for transaction in tx_ids {
            let node = self.nodes.remove(transaction).expect("Node must be in graph");
            assert_eq!(node.status, Status::Processed);

            transactions.push(node.data);

            // Remove node from graph. No need to update parents as they should be removed in this
            // call as well.
            for child in node.children {
                // Its possible for the child to part of this same set of batches and therefore
                // already removed.
                if let Some(child) = self.nodes.get_mut(&child) {
                    child.parents.remove(transaction);
                }
            }
        }

        transactions
    }

    /// Removes the transactions and all their descendants from the graph.
    ///
    /// Returns all transactions removed.
    pub fn purge_subgraphs(&mut self, transactions: Vec<TransactionId>) -> Vec<T> {
        let mut removed = Vec::new();

        let mut to_process = transactions;

        while let Some(node_id) = to_process.pop() {
            // Its possible for a node to already have been removed as part of this subgraph
            // removal.
            let Some(node) = self.nodes.remove(&node_id) else {
                continue;
            };

            // All the children will also be removed so no need to check for new roots.
            //
            // No new roots are possible as a result of this subgraph removal.
            self.roots.remove(&node_id);

            // Inform parent that this child no longer exists.
            //
            // The same is not required for children of this batch as we will
            // be removing those as well.
            for parent in &node.parents {
                // Parent could already be removed as part of this subgraph removal.
                if let Some(parent) = self.nodes.get_mut(parent) {
                    parent.children.remove(&node_id);
                }
            }

            to_process.extend(node.children);
            removed.push(node.data);
        }

        removed
    }

    /// Adds the given transaction to the set of roots _IFF_ all of its parents are marked as
    /// processed.
    ///
    /// # Panics
    ///
    /// Panics if the transaction or any of its parents do not exist. This would constitute an
    /// internal bookkeeping failure.
    fn try_make_root(&mut self, tx_id: TransactionId) {
        let tx = self.nodes.get_mut(&tx_id).expect("Transaction must be in graph");

        for parent in tx.parents.clone() {
            let parent = self.nodes.get(&parent).expect("Parent must be in pool");

            if !parent.is_processed() {
                return;
            }
        }
        self.roots.insert(tx_id);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Node<T> {
    status: Status,
    data: T,
    parents: BTreeSet<TransactionId>,
    children: BTreeSet<TransactionId>,
}

impl<T> Node<T> {
    /// Creates a new inflight [Node] with no children.
    fn new(data: T, parents: BTreeSet<TransactionId>) -> Self {
        Self {
            status: Status::Queued,
            data,
            parents,
            children: Default::default(),
        }
    }

    /// Marks the node as [Status::Processed].
    ///
    /// # Panics
    ///
    /// Panics if the node is already processed.
    fn mark_as_processed(&mut self) {
        assert!(!self.is_processed());
        self.status = Status::Processed
    }

    /// Marks the node as [Status::Inqueue].
    ///
    /// # Panics
    ///
    /// Panics if the node is already inqueue.
    fn mark_as_inqueue(&mut self) {
        assert!(!self.is_inqueue());
        self.status = Status::Queued
    }

    fn is_processed(&self) -> bool {
        self.status == Status::Processed
    }

    fn is_inqueue(&self) -> bool {
        self.status == Status::Queued
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Queued,
    Processed,
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::Random;

    /// Simplified graph type which uses the transaction ID as the data value.
    ///
    /// Production usage will have `T: ProvenTransaction` however this is cumbersome
    /// to generate. Since this graph doesn't actually care about the data type, we
    /// simplify test data generation by just duplicating the ID.
    type TestGraph = TransactionGraph<TransactionId>;

    /// Test helpers and aliases.
    impl TestGraph {
        /// Alias to insert a transaction with no parents.
        fn insert_with_no_parent(&mut self, id: TransactionId) {
            self.insert_with_parents(id, Default::default());
        }

        /// Alias for inserting a transaction with parents.
        fn insert_with_parents(&mut self, id: TransactionId, parents: BTreeSet<TransactionId>) {
            self.insert(id, id, parents);
        }

        /// Alias for inserting a transaction with a single parent.
        fn insert_with_parent(&mut self, id: TransactionId, parent: TransactionId) {
            self.insert_with_parents(id, [parent].into());
        }

        /// Calls `pop_for_processing` until it returns `None`.
        ///
        /// This should result in a fully processed graph, barring bugs.
        ///
        /// Panics if the graph is not fully processed.
        fn process_all(&mut self) -> Vec<TransactionId> {
            let mut processed = Vec::new();
            while let Some((id, _)) = self.pop_for_processing() {
                processed.push(id);
            }

            assert!(self.nodes.values().all(Node::is_processed));

            processed
        }
    }

    #[test]
    fn pruned_nodes_are_nonextant() {
        //! Checks that processed and then pruned nodes behave as if they
        //! never existed in the graph. We test this by comparing it to
        //! a reference graph created without these ancestor nodes.
        let mut rng = Random::with_random_seed();

        let ancestor_a = rng.draw_tx_id();
        let ancestor_b = rng.draw_tx_id();

        let child_a = rng.draw_tx_id();
        let child_b = rng.draw_tx_id();
        let child_both = rng.draw_tx_id();

        let mut uut = TestGraph::default();
        uut.insert_with_no_parent(ancestor_a);
        uut.insert_with_no_parent(ancestor_b);
        uut.insert_with_parent(child_a, ancestor_a);
        uut.insert_with_parent(child_b, ancestor_b);
        uut.insert_with_parents(child_both, [ancestor_a, ancestor_b].into());

        uut.process(ancestor_a);
        uut.process(ancestor_b);
        uut.prune_processed(&[ancestor_a, ancestor_b]);

        let mut reference = TestGraph::default();
        reference.insert_with_no_parent(child_a);
        reference.insert_with_no_parent(child_b);
        reference.insert_with_no_parent(child_both);

        assert_eq!(uut, reference);
    }

    #[test]
    fn inserted_node_is_considered_for_root() {
        //! Ensure that a fresh node who's parent is
        //! already processed will be considered for processing.
        let mut rng = Random::with_random_seed();
        let parent_a = rng.draw_tx_id();
        let parent_b = rng.draw_tx_id();
        let child_a = rng.draw_tx_id();
        let child_b = rng.draw_tx_id();

        let mut uut = TestGraph::default();
        uut.insert_with_no_parent(parent_a);
        uut.insert_with_no_parent(parent_b);
        uut.process(parent_a);

        uut.insert_with_parent(child_a, parent_a);
        uut.insert_with_parent(child_b, parent_b);

        assert!(uut.roots.contains(&child_a));
        assert!(!uut.roots.contains(&child_b));
    }

    #[test]
    fn fifo_order_is_maintained() {
        //! This test creates a simple queue graph, expecting that the processed items should
        //! be emitted in the same order.
        let mut rng = Random::with_random_seed();
        let input = (0..10).map(|_| rng.draw_tx_id()).collect::<Vec<_>>();

        let mut uut = TestGraph::default();
        uut.insert_with_no_parent(input[0]);
        for pairs in input.windows(2) {
            let (parent, id) = (pairs[0], pairs[1]);
            uut.insert_with_parent(id, parent);
        }

        let result = uut.process_all();
        assert_eq!(result, input);
    }

    #[test]
    fn requeuing_resets_graph_state() {
        //! Requeuing transactions should cause the internal state to reset
        //! to the same state as before these transactions were emitted
        //! for processing.

        let mut rng = Random::with_random_seed();

        let ancestor_a = rng.draw_tx_id();
        let ancestor_b = rng.draw_tx_id();
        let parent_a = rng.draw_tx_id();
        let parent_b = rng.draw_tx_id();
        let child_a = rng.draw_tx_id();
        let child_b = rng.draw_tx_id();
        let child_c = rng.draw_tx_id();

        let mut uut = TestGraph::default();
        uut.insert_with_no_parent(ancestor_a);
        uut.insert_with_no_parent(ancestor_b);
        uut.insert_with_parent(parent_a, ancestor_a);
        uut.insert_with_parent(parent_b, ancestor_b);
        uut.insert_with_parents(child_a, [ancestor_a, parent_a].into());
        uut.insert_with_parents(child_b, [parent_a, parent_b].into());
        uut.insert_with_parent(child_c, parent_b);

        let mut reference = uut.clone();

        uut.process(ancestor_a);
        uut.process(ancestor_b);
        uut.process(parent_a);
        uut.process(parent_b);
        uut.process(child_c);

        // Requeue all except ancestor a. This is a somewhat arbitrary choice.
        // The reference graph should therefore only have ancestor a processed.
        uut.requeue_transactions([ancestor_b, parent_a, parent_b, child_c].into());
        reference.process(ancestor_a);

        assert_eq!(uut, reference);
    }

    #[test]
    fn nodes_are_processed_exactly_once() {
        let mut rng = Random::with_random_seed();

        let ancestor_a = rng.draw_tx_id();
        let ancestor_b = rng.draw_tx_id();
        let parent_a = rng.draw_tx_id();
        let parent_b = rng.draw_tx_id();
        let child_a = rng.draw_tx_id();
        let child_b = rng.draw_tx_id();
        let child_c = rng.draw_tx_id();

        let mut uut = TestGraph::default();
        uut.insert_with_no_parent(ancestor_a);
        uut.insert_with_no_parent(ancestor_b);
        uut.insert_with_parent(parent_a, ancestor_a);
        uut.insert_with_parent(parent_b, ancestor_b);
        uut.insert_with_parents(child_a, [ancestor_a, parent_a].into());
        uut.insert_with_parents(child_b, [parent_a, parent_b].into());
        uut.insert_with_parent(child_c, parent_b);

        let mut result = uut.process_all();
        result.sort();

        let mut expected =
            vec![ancestor_a, ancestor_b, parent_a, parent_b, child_a, child_b, child_c];
        expected.sort();

        assert_eq!(result, expected);
    }

    #[test]
    fn processed_data_and_parent_tracking() {
        let mut rng = Random::with_random_seed();

        let ancestor_a = rng.draw_tx_id();
        let ancestor_b = rng.draw_tx_id();
        let parent_a = rng.draw_tx_id();
        let parent_b = rng.draw_tx_id();
        let child_a = rng.draw_tx_id();
        let child_b = rng.draw_tx_id();
        let child_c = rng.draw_tx_id();

        let mut uut = TestGraph::default();
        uut.insert_with_no_parent(ancestor_a);
        uut.insert_with_no_parent(ancestor_b);
        uut.insert_with_parent(parent_a, ancestor_a);
        uut.insert_with_parent(parent_b, ancestor_b);
        uut.insert_with_parents(child_a, [ancestor_a, parent_a].into());
        uut.insert_with_parents(child_b, [parent_a, parent_b].into());
        uut.insert_with_parent(child_c, parent_b);

        let result = uut.process(ancestor_a);
        assert_eq!(result, (ancestor_a, Default::default()));

        let result = uut.process(ancestor_b);
        assert_eq!(result, (ancestor_b, Default::default()));

        let result = uut.process(parent_a);
        assert_eq!(result, (parent_a, [ancestor_a].into()));

        let result = uut.process(parent_b);
        assert_eq!(result, (parent_b, [ancestor_b].into()));

        let result = uut.process(child_a);
        assert_eq!(result, (child_a, [ancestor_a, parent_a].into()));

        let result = uut.process(child_b);
        assert_eq!(result, (child_b, [parent_a, parent_b].into()));

        let result = uut.process(child_c);
        assert_eq!(result, (child_c, [parent_b].into()));
    }

    #[test]
    fn purging_subgraph_handles_internal_nodes() {
        //! Purging a subgraph should correctly handle nodes already deleted within that subgraph.
        //!
        //! This is a concern for errors as we are deleting parts of the subgraph while we are
        //! iterating through the nodes to purge. This means its likely a node will already
        //! have been deleted before processing it as an input.
        //!
        //! We can somewhat force this to occur by re-ordering the inputs relative to the actual
        //! dependency order.

        let mut rng = Random::with_random_seed();

        let ancestor_a = rng.draw_tx_id();
        let ancestor_b = rng.draw_tx_id();
        let parent_a = rng.draw_tx_id();
        let parent_b = rng.draw_tx_id();
        let child_a = rng.draw_tx_id();
        let child_b = rng.draw_tx_id();
        let child_c = rng.draw_tx_id();

        let mut uut = TestGraph::default();
        uut.insert_with_no_parent(ancestor_a);
        uut.insert_with_no_parent(ancestor_b);
        uut.insert_with_parent(parent_a, ancestor_a);
        uut.insert_with_parent(parent_b, ancestor_b);
        uut.insert_with_parents(child_a, [ancestor_a, parent_a].into());
        uut.insert_with_parents(child_b, [parent_a, parent_b].into());
        uut.insert_with_parent(child_c, parent_b);

        uut.purge_subgraphs(vec![child_b, parent_a]);

        let mut reference = TestGraph::default();
        reference.insert_with_no_parent(ancestor_a);
        reference.insert_with_no_parent(ancestor_b);
        reference.insert_with_parent(parent_b, ancestor_b);
        reference.insert_with_parent(child_c, parent_b);

        assert_eq!(uut, reference);
    }

    #[test]
    fn purging_removes_all_descendents() {
        let mut rng = Random::with_random_seed();

        let ancestor_a = rng.draw_tx_id();
        let ancestor_b = rng.draw_tx_id();
        let parent_a = rng.draw_tx_id();
        let parent_b = rng.draw_tx_id();
        let child_a = rng.draw_tx_id();
        let child_b = rng.draw_tx_id();
        let child_c = rng.draw_tx_id();

        let mut uut = TestGraph::default();
        uut.insert_with_no_parent(ancestor_a);
        uut.insert_with_no_parent(ancestor_b);
        uut.insert_with_parent(parent_a, ancestor_a);
        uut.insert_with_parent(parent_b, ancestor_b);
        uut.insert_with_parents(child_a, [ancestor_a, parent_a].into());
        uut.insert_with_parents(child_b, [parent_a, parent_b].into());
        uut.insert_with_parent(child_c, parent_b);

        uut.purge_subgraphs(vec![parent_a]);

        let mut reference = TestGraph::default();
        reference.insert_with_no_parent(ancestor_a);
        reference.insert_with_no_parent(ancestor_b);
        reference.insert_with_parent(parent_b, ancestor_b);
        reference.insert_with_parent(child_c, parent_b);

        assert_eq!(uut, reference);
    }

    #[test]
    #[should_panic]
    fn duplicate_insert() {
        let mut rng = Random::with_random_seed();
        let mut uut = TestGraph::default();

        let id = rng.draw_tx_id();
        uut.insert_with_no_parent(id);
        uut.insert_with_no_parent(id);
    }

    #[test]
    #[should_panic]
    fn missing_parents_in_insert() {
        let mut rng = Random::with_random_seed();
        let mut uut = TestGraph::default();

        uut.insert_with_parents(rng.draw_tx_id(), [rng.draw_tx_id()].into());
    }

    #[test]
    #[should_panic]
    fn requeueing_an_already_queued_tx() {
        let mut rng = Random::with_random_seed();
        let mut uut = TestGraph::default();

        let id = rng.draw_tx_id();
        uut.insert_with_no_parent(id);
        uut.requeue_transactions([id].into());
    }
}
