use miden_objects::accounts::AccountId;
use miden_vm::{crypto::MerkleError, ExecutionError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildBlockError {
    #[error("failed to update account root")]
    AccountRootUpdateFailed(BlockProverError),
    #[error("transaction batches and store don't modify the same account IDs. Offending accounts: {0:?}")]
    InconsistentAccountIds(Vec<AccountId>),
    #[error("transaction batches and store contain different hashes for some accounts. Offending accounts: {0:?}")]
    InconsistentAccountStates(Vec<AccountId>),
    #[error("dummy")]
    Dummy,
}

impl From<BlockProverError> for BuildBlockError {
    fn from(err: BlockProverError) -> Self {
        Self::AccountRootUpdateFailed(err)
    }
}

#[derive(Error, Debug)]
pub enum BlockProverError {
    #[error("Received invalid merkle path")]
    InvalidMerklePaths(MerkleError),
    #[error("program execution failed")]
    ProgramExecutionFailed(ExecutionError),
    #[error("invalid return value on stack (not a hash)")]
    InvalidRootReturned,
}
