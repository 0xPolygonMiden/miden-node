use miden_objects::{accounts::AccountId, Digest};
use miden_vm::{crypto::MerkleError, ExecutionError};
use thiserror::Error;

use super::prover::block_witness::CREATED_NOTES_TREE_INSERTION_DEPTH;
use crate::store::{ApplyBlockError, BlockInputsError};

#[derive(Debug, Error, PartialEq)]
pub enum BuildBlockError {
    #[error("failed to compute new block: {0}")]
    BlockProverFailed(#[from] BlockProverError),
    #[error("failed to apply block: {0}")]
    ApplyBlockFailed(#[from] ApplyBlockError),
    #[error("failed to get block inputs from store: {0}")]
    GetBlockInputsFailed(#[from] BlockInputsError),
    #[error("transaction batches and store don't modify the same account IDs. Offending accounts: {0:?}")]
    InconsistentAccountIds(Vec<AccountId>),
    #[error("transaction batches and store contain different hashes for some accounts. Offending accounts: {0:?}")]
    InconsistentAccountStates(Vec<AccountId>),
    #[error("transaction batches and store don't produce the same nullifiers. Offending nullifiers: {0:?}")]
    InconsistentNullifiers(Vec<Digest>),
    #[error(
        "too many batches in block. Got: {0}, max: 2^{}",
        CREATED_NOTES_TREE_INSERTION_DEPTH
    )]
    TooManyBatchesInBlock(usize),
}

#[derive(Error, Debug, PartialEq)]
pub enum BlockProverError {
    #[error("Received invalid merkle path")]
    InvalidMerklePaths(MerkleError),
    #[error("program execution failed")]
    ProgramExecutionFailed(ExecutionError),
    #[error("failed to retrieve {0} root from stack outputs")]
    InvalidRootOutput(String),
}
