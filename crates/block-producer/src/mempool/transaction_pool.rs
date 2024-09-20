use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use miden_objects::transaction::{ProvenTransaction, TransactionId};

use super::BatchId;

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

        // This could be optimised by inlining this inside the parent loop. This would prevent the double iteration over parents, at the cost of some code duplication.
        self.try_make_root(id);
    }

    /// Removes the transaction from the graph.
    pub fn remove(&mut self, tx_id: &TransactionId) {
        if let Some(transaction) = self.nodes.remove(tx_id) {
            for parent in transaction.parents {
                self.nodes
                    .get_mut(&parent)
                    .expect("Parent must be in pool")
                    .children
                    .remove(tx_id);
            }
        }
    }

    pub fn pop_for_batching(&mut self) -> Option<Arc<ProvenTransaction>> {
        let transaction = self.roots.pop_first()?;
        let transaction =
            self.nodes.get_mut(&transaction).expect("Root transaction must be in graph");

        transaction.status = Status::Processed;

        Some(Arc::clone(&transaction.data))
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
