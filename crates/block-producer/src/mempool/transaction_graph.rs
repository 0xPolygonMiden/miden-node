use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use miden_objects::transaction::{ProvenTransaction, TransactionId};

use super::BatchJobId;

#[derive(Default, Clone, Debug)]
pub struct TransactionGraph {
    /// All transactions currently inflight.
    nodes: BTreeMap<TransactionId, Node>,

    /// Transactions ready for inclusion in a batch.
    ///
    /// aka transactions whose parent transactions are already included in batches.
    roots: BTreeSet<TransactionId>,
}
impl TransactionGraph {
    pub fn insert(&mut self, transaction: ProvenTransaction, parents: BTreeSet<TransactionId>) {
        let id = transaction.id();

        // Inform parent's of their new child.
        for parent in &parents {
            self.nodes.get_mut(&parent).expect("Parent must be in pool").children.insert(id);
        }

        let transaction = Node {
            status: Status::InQueue,
            data: Arc::new(transaction),
            parents,
            children: Default::default(),
        };
        if self.nodes.insert(id, transaction).is_some() {
            panic!("Transaction already exists in pool");
        }

        // This could be optimised by inlining this inside the parent loop. This would prevent the
        // double iteration over parents, at the cost of some code duplication.
        self.try_make_root(id);
    }

    pub fn pop_for_batching(
        &mut self,
    ) -> Option<(Arc<ProvenTransaction>, BTreeSet<TransactionId>)> {
        let transaction = self.roots.pop_first()?;
        let transaction =
            self.nodes.get_mut(&transaction).expect("Root transaction must be in graph");
        transaction.status = Status::Processed;

        // Work around multiple mutable borrows of self.
        let data = Arc::clone(&transaction.data);
        let parents = transaction.parents.clone();
        let children = transaction.children.clone();

        for child in children {
            self.try_make_root(child);
        }

        Some((data, parents))
    }

    /// Marks the given transactions as being back inqueue.
    pub fn requeue_transactions(&mut self, transactions: BTreeSet<TransactionId>) {
        for tx in &transactions {
            self.nodes.get_mut(&tx).expect("Node must exist").status = Status::InQueue;
        }

        // All requeued transactions are potential roots, and current roots may have been
        // invalidated.
        let mut potential_roots = transactions;
        potential_roots.extend(&self.roots);
        self.roots.clear();
        for tx in potential_roots {
            self.try_make_root(tx);
        }
    }

    pub fn remove_stale(&mut self, transactions: Vec<TransactionId>) {
        for transaction in transactions {
            let node = self.nodes.remove(&transaction).expect("Node must be in graph");
            assert_eq!(node.status, Status::Processed);

            // Remove node from graph. No need to update parents as they should be removed in this
            // call as well.
            for child in node.children {
                // Its possible for the child to part of this same set of batches and therefore
                // already removed.
                if let Some(child) = self.nodes.get_mut(&child) {
                    child.parents.remove(&transaction);
                }
            }
        }
    }

    fn try_make_root(&mut self, tx_id: TransactionId) {
        let tx = self.nodes.get_mut(&tx_id).expect("Transaction must be in graph");

        for parent in tx.parents.clone() {
            let parent = self.nodes.get(&parent).expect("Parent must be in pool");

            if parent.status != Status::Processed {
                return;
            }
        }
        self.roots.insert(tx_id);
    }
}

#[derive(Clone, Debug)]
struct Node {
    status: Status,
    data: Arc<ProvenTransaction>,
    parents: BTreeSet<TransactionId>,
    children: BTreeSet<TransactionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    InQueue,
    Processed,
}
