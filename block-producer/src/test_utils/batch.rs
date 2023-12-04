use std::sync::Arc;

use crate::{batch_builder::TransactionBatch, test_utils::MockProvenTxBuilder};

pub trait TransactionBatchConstructor {
    /// Returns a `TransactionBatch` with `notes_per_tx.len()` transactions, where the i'th
    /// transaction has `notes_per_tx[i]` notes created
    fn from_notes_created(notes_per_tx: &[u64]) -> Self;

    /// Returns a `TransactionBatch` which contains `num_txs_in_batch` transactions
    fn from_txs(num_txs_in_batch: u64) -> Self;
}

impl TransactionBatchConstructor for TransactionBatch {
    fn from_notes_created(notes_per_tx: &[u64]) -> Self {
        let txs: Vec<_> = notes_per_tx
            .iter()
            .map(|&num_notes| MockProvenTxBuilder::new().num_notes_created(num_notes).build())
            .map(Arc::new)
            .collect();

        Self::new(txs).unwrap()
    }

    fn from_txs(num_txs_in_batch: u64) -> Self {
        let txs: Vec<_> = (0..num_txs_in_batch)
            .map(|_| MockProvenTxBuilder::new().build())
            .map(Arc::new)
            .collect();

        Self::new(txs).unwrap()
    }
}
