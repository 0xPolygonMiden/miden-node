use miden_objects::accounts::AccountId;
use miden_vm::{crypto::MerkleError, ExecutionError};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BuildBlockError {
    #[error("failed to update account root")]
    AccountRootUpdateFailed(#[from] BlockProverError),
    #[error("transaction batches and store don't modify the same account IDs. Offending accounts: {0:?}")]
    InconsistentAccountIds(Vec<AccountId>),
    #[error("transaction batches and store contain different hashes for some accounts. Offending accounts: {0:?}")]
    InconsistentAccountStates(Vec<AccountId>),
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
