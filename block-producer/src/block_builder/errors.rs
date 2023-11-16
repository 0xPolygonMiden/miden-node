use miden_objects::accounts::AccountId;
use miden_vm::{crypto::MerkleError, ExecutionError};
use thiserror::Error;

use crate::store::{ApplyBlockError, BlockInputsError};

use super::prover::CREATED_NOTES_TREE_INSERTION_DEPTH;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BuildBlockError {
    #[error("failed to apply block: {0}")]
    ApplyBlockFailed(#[from] ApplyBlockError),
    #[error("failed to compute new block header: {0}")]
    ComputeBlockHeaderFailed(#[from] BlockProverError),
    #[error("failed to get block inputs from store: {0}")]
    GetBlockInputsFailed(#[from] BlockInputsError),
    #[error("transaction batches and store don't modify the same account IDs. Offending accounts: {0:?}")]
    InconsistentAccountIds(Vec<AccountId>),
    #[error("transaction batches and store contain different hashes for some accounts. Offending accounts: {0:?}")]
    InconsistentAccountStates(Vec<AccountId>),
    #[error(
        "Too many batches in block. Got: {0}, max: 2^{}",
        CREATED_NOTES_TREE_INSERTION_DEPTH
    )]
    TooManyBatchesInBlock(usize),
    #[error("dummy")]
    Dummy,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum BlockProverError {
    #[error("Received invalid merkle path")]
    InvalidMerklePaths(MerkleError),
    #[error("program execution failed")]
    ProgramExecutionFailed(ExecutionError),
    #[error("invalid return value on stack (not a hash)")]
    InvalidRootReturned,
}
