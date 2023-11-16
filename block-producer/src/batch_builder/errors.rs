use miden_vm::crypto::MerkleError;
use thiserror::Error;

use super::MAX_NUM_CREATED_NOTES_PER_BATCH;

#[derive(Error, Debug, PartialEq)]
pub enum BuildBatchError {
    #[error("dummy")]
    Dummy,
    #[error(
        "Too many notes in the batch. Got: {0}, max: {}",
        MAX_NUM_CREATED_NOTES_PER_BATCH
    )]
    TooManyNotesCreated(usize),
    #[error("failed to create notes SMT: {0}")]
    NotesSmtError(#[from] MerkleError),
}
