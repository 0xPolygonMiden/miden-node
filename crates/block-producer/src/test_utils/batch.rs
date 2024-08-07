use crate::{test_utils::MockProvenTxBuilder, TransactionBatch};

pub trait TransactionBatchConstructor {
    /// Returns a `TransactionBatch` with `notes_per_tx.len()` transactions, where the i'th
    /// transaction has `notes_per_tx[i]` notes created
    fn from_notes_created(starting_account_index: u32, notes_per_tx: &[u64]) -> Self;

    /// Returns a `TransactionBatch` which contains `num_txs_in_batch` transactions
    fn from_txs(starting_account_index: u32, num_txs_in_batch: u64) -> Self;
}

impl TransactionBatchConstructor for TransactionBatch {
    fn from_notes_created(starting_account_index: u32, notes_per_tx: &[u64]) -> Self {
        let txs: Vec<_> = notes_per_tx
            .iter()
            .enumerate()
            .map(|(index, &num_notes)| {
                let starting_note_index = starting_account_index as u64 + index as u64;
                MockProvenTxBuilder::with_account_index(starting_account_index + index as u32)
                    .private_notes_created_range(
                        starting_note_index..(starting_note_index + num_notes),
                    )
                    .build()
            })
            .collect();

        Self::new(txs, Default::default()).unwrap()
    }

    fn from_txs(starting_account_index: u32, num_txs_in_batch: u64) -> Self {
        let txs: Vec<_> = (0..num_txs_in_batch)
            .enumerate()
            .map(|(index, _)| {
                MockProvenTxBuilder::with_account_index(starting_account_index + index as u32)
                    .build()
            })
            .collect();

        Self::new(txs, Default::default()).unwrap()
    }
}
