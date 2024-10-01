use std::collections::{BTreeMap, BTreeSet};

use miden_objects::transaction::TransactionId;
use miden_tx::utils::collections::KvMap;

use super::BatchJobId;

#[derive(Default, Clone)]
pub struct BatchGraph {
    nodes: BTreeMap<BatchJobId, Node>,
    roots: BTreeSet<BatchJobId>,

    /// Allows for reverse lookup of transaction -> batch.
    transactions: BTreeMap<TransactionId, BatchJobId>,
}

impl BatchGraph {
    pub fn insert(
        &mut self,
        id: BatchJobId,
        transactions: Vec<TransactionId>,
        parents: BTreeSet<TransactionId>,
    ) {
        // Reverse lookup parent transaction batches.
        let parents = parents
            .into_iter()
            .map(|tx| self.transactions.get(&tx).expect("Parent transaction must be in a batch"))
            .copied()
            .collect();

        // Inform parents of their new child.
        for parent in &parents {
            self.nodes
                .get_mut(parent)
                .expect("Parent batch must be present")
                .children
                .insert(id);
        }

        // Insert transactions for reverse lookup.
        for tx in &transactions {
            self.transactions.insert(*tx, id);
        }

        // Insert the new node into the graph.
        let batch = Node {
            status: Status::InFlight,
            transactions,
            parents,
            children: Default::default(),
        };
        self.nodes.insert(id, batch);

        // New node might be a root.
        //
        // This could be optimised by inlining this inside the parent loop. This would prevent the
        // double iteration over parents, at the cost of some code duplication.
        self.try_make_root(id);
    }

    /// Removes the batches and all their descendents from the graph.
    ///
    /// Returns all removed batches and their transactions.
    pub fn purge_subgraphs(
        &mut self,
        batches: Vec<BatchJobId>,
    ) -> Vec<(BatchJobId, Vec<TransactionId>)> {
        let mut removed = Vec::new();

        let mut to_process = batches;

        while let Some(node_id) = to_process.pop() {
            // Its possible for a node to already have been removed as part of this subgraph
            // removal.
            let Some(node) = self.nodes.remove(&node_id) else {
                continue;
            };

            // All the child batches are also removed so no need to check
            // for new roots. No new roots are possible as a result of this subgraph removal.
            self.roots.remove(&node_id);

            for transaction in &node.transactions {
                self.transactions.remove(transaction);
            }

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
            removed.push((node_id, node.transactions));
        }

        removed
    }

    /// Removes a set of batches from the graph without removing any descendents.
    ///
    /// This is intended to cull completed batches from stale blocks.
    pub fn remove_stale(&mut self, batches: Vec<BatchJobId>) -> Vec<TransactionId> {
        let mut transactions = Vec::new();

        for batch in batches {
            let node = self.nodes.remove(&batch).expect("Node must be in graph");
            assert_eq!(node.status, Status::InBlock);

            // Remove batch from graph. No need to update parents as they should be removed in this
            // call as well.
            for child in node.children {
                // Its possible for the child to part of this same set of batches and therefore
                // already removed.
                if let Some(child) = self.nodes.get_mut(&child) {
                    child.parents.remove(&batch);
                }
            }

            transactions.extend_from_slice(&node.transactions);
        }

        transactions
    }

    /// Mark a batch as proven if it exists.
    pub fn mark_proven(&mut self, id: BatchJobId) {
        // Its possible for inflight batches to have been removed as part
        // of another batches failure.
        if let Some(node) = self.nodes.get_mut(&id) {
            node.status = Status::Proven;
            self.try_make_root(id);
        }
    }

    pub fn pop_for_blocking(&mut self) -> Option<(BatchJobId, Vec<TransactionId>)> {
        let batch_id = self.roots.pop_first()?;
        let node = self.nodes.get_mut(&batch_id).expect("Root node must be in graph");
        node.status = Status::InBlock;

        // Work around multiple mutable borrows of self.
        let transactions = node.transactions.clone();
        let children = node.children.clone();

        for child in children {
            self.try_make_root(child);
        }

        Some((batch_id, transactions))
    }

    fn try_make_root(&mut self, id: BatchJobId) {
        let node = self.nodes.get_mut(&id).expect("Node must be in graph");

        for parent in node.parents.clone() {
            let parent = self.nodes.get(&parent).expect("Parent must be in pool");

            if parent.status != Status::InBlock {
                return;
            }
        }
        self.roots.insert(id);
    }
}

#[derive(Clone, Debug)]
struct Node {
    status: Status,
    transactions: Vec<TransactionId>,
    parents: BTreeSet<BatchJobId>,
    children: BTreeSet<BatchJobId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    InFlight,
    Proven,
    InBlock,
}
