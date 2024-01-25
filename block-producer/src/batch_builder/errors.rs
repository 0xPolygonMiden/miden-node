use miden_objects::transaction::ProvenTransaction;
use miden_vm::crypto::MerkleError;
use thiserror::Error;

use crate::MAX_NUM_CREATED_NOTES_PER_BATCH;

/// Error that may happen while building a transaction batch.
///
/// These errors are returned from the batch builder to the transaction queue, instead of
/// dropping the transactions, they are included into the error values, so that the transaction
/// queue can re-queue them.
#[derive(Error, Debug)]
pub enum BuildBatchError {
    #[error(
        "Too many notes in the batch. Got: {0}, max: {}",
        MAX_NUM_CREATED_NOTES_PER_BATCH
    )]
    TooManyNotesCreated(usize, Vec<ProvenTransaction>),

    #[error("failed to create notes SMT: {0}")]
    NotesSmtError(MerkleError, Vec<ProvenTransaction>),
}

impl BuildBatchError {
    pub fn into_transactions(self) -> Vec<ProvenTransaction> {
        match self {
            BuildBatchError::TooManyNotesCreated(_, txs) => txs,
            BuildBatchError::NotesSmtError(_, txs) => txs,
        }
    }
}
