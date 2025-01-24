use std::io;

use deadpool_sqlite::{InteractError, PoolError};
use miden_objects::{
    account::AccountId,
    block::{BlockHeader, BlockNumber},
    crypto::{
        hash::rpo::RpoDigest,
        merkle::{MerkleError, MmrError},
        utils::DeserializationError,
    },
    note::Nullifier,
    transaction::OutputNote,
    AccountDeltaError, AccountError, BlockError, NoteError,
};
use rusqlite::types::FromSqlError;
use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;
use tonic::Status;

// INTERNAL ERRORS
// =================================================================================================

#[derive(Debug, Error)]
pub enum NullifierTreeError {
    #[error("failed to create nullifier tree")]
    CreationFailed(#[source] MerkleError),

    #[error("failed to mutate nullifier tree")]
    MutationFailed(#[source] MerkleError),
}

// DATABASE ERRORS
// =================================================================================================

#[derive(Debug, Error)]
pub enum DatabaseError {
    // ERRORS WITH AUTOMATIC CONVERSIONS FROM NESTED ERROR TYPES
    // ---------------------------------------------------------------------------------------------
    #[error("account error")]
    AccountError(#[from] AccountError),
    #[error("account delta error")]
    AccountDeltaError(#[from] AccountDeltaError),
    #[error("block error")]
    BlockError(#[from] BlockError),
    #[error("closed channel")]
    ClosedChannel(#[from] RecvError),
    #[error("deserialization failed")]
    DeserializationError(#[from] DeserializationError),
    #[error("hex parsing error")]
    FromHexError(#[from] hex::FromHexError),
    #[error("SQLite deserialization error")]
    FromSqlError(#[from] FromSqlError),
    #[error("I/O error")]
    IoError(#[from] io::Error),
    #[error("migration failed")]
    MigrationError(#[from] rusqlite_migration::Error),
    #[error("missing database connection")]
    MissingDbConnection(#[from] PoolError),
    #[error("merkle error")]
    MerkleError(#[from] MerkleError),
    #[error("note error")]
    NoteError(#[from] NoteError),
    #[error("SQLite error")]
    SqliteError(#[from] rusqlite::Error),

    // OTHER ERRORS
    // ---------------------------------------------------------------------------------------------
    #[error("account hash mismatch (expected {expected}, but calculated is {calculated})")]
    AccountHashesMismatch {
        expected: RpoDigest,
        calculated: RpoDigest,
    },
    #[error("account {0} not found")]
    AccountNotFoundInDb(AccountId),
    #[error("accounts {0:?} not found")]
    AccountsNotFoundInDb(Vec<AccountId>),
    #[error("account {0} is not on the chain")]
    AccountNotPublic(AccountId),
    #[error("block {0} not found")]
    BlockNotFoundInDb(BlockNumber),
    #[error("data corrupted: {0}")]
    DataCorrupted(String),
    #[error("SQLite pool interaction failed: {0}")]
    InteractError(String),
    #[error("invalid Felt: {0}")]
    InvalidFelt(String),
    #[error(
        "unsupported database version. There is no migration chain from/to this version. \
        Remove all database files and try again."
    )]
    UnsupportedDatabaseVersion,
}

impl From<DatabaseError> for Status {
    fn from(err: DatabaseError) -> Self {
        match err {
            DatabaseError::AccountNotFoundInDb(_)
            | DatabaseError::AccountsNotFoundInDb(_)
            | DatabaseError::AccountNotPublic(_)
            | DatabaseError::BlockNotFoundInDb(_) => Status::not_found(err.to_string()),

            _ => Status::internal(err.to_string()),
        }
    }
}

// INITIALIZATION ERRORS
// =================================================================================================

#[derive(Error, Debug)]
pub enum StateInitializationError {
    #[error("database error")]
    DatabaseError(#[from] DatabaseError),
    #[error("failed to create nullifier tree")]
    FailedToCreateNullifierTree(#[from] NullifierTreeError),
    #[error("failed to create accounts tree")]
    FailedToCreateAccountsTree(#[from] MerkleError),
}

#[derive(Debug, Error)]
pub enum DatabaseSetupError {
    #[error("I/O error")]
    IoError(#[from] io::Error),
    #[error("database error")]
    DatabaseError(#[from] DatabaseError),
    #[error("genesis block error")]
    GenesisBlockError(#[from] GenesisError),
    #[error("pool build error")]
    PoolBuildError(#[from] deadpool_sqlite::BuildError),
    #[error("SQLite migration error")]
    SqliteMigrationError(#[from] rusqlite_migration::Error),
}

#[derive(Debug, Error)]
pub enum GenesisError {
    // ERRORS WITH AUTOMATIC CONVERSIONS FROM NESTED ERROR TYPES
    // ---------------------------------------------------------------------------------------------
    #[error("database error")]
    DatabaseError(#[from] DatabaseError),
    #[error("block error")]
    BlockError(#[from] BlockError),
    #[error("merkle error")]
    MerkleError(#[from] MerkleError),
    #[error("failed to deserialize genesis file")]
    GenesisFileDeserializationError(#[from] DeserializationError),
    #[error("retrieving genesis block header failed")]
    SelectBlockHeaderByBlockNumError(#[from] Box<DatabaseError>),

    // OTHER ERRORS
    // ---------------------------------------------------------------------------------------------
    #[error("apply block failed")]
    ApplyBlockFailed(#[source] InteractError),
    #[error("failed to read genesis file \"{genesis_filepath}\"")]
    FailedToReadGenesisFile {
        genesis_filepath: String,
        source: io::Error,
    },
    #[error("block header in store doesn't match block header in genesis file. Expected {expected_genesis_header:?}, but store contained {block_header_in_store:?}")]
    GenesisBlockHeaderMismatch {
        expected_genesis_header: Box<BlockHeader>,
        block_header_in_store: Box<BlockHeader>,
    },
}

// ENDPOINT ERRORS
// =================================================================================================
#[derive(Error, Debug)]
pub enum InvalidBlockError {
    #[error("duplicated nullifiers {0:?}")]
    DuplicatedNullifiers(Vec<Nullifier>),
    #[error("invalid output note type: {0:?}")]
    InvalidOutputNoteType(Box<OutputNote>),
    #[error("invalid block tx hash: expected {expected}, but got {actual}")]
    InvalidBlockTxHash { expected: RpoDigest, actual: RpoDigest },
    #[error("received invalid account tree root")]
    NewBlockInvalidAccountRoot,
    #[error("new block number must be 1 greater than the current block number")]
    NewBlockInvalidBlockNum,
    #[error("new block chain root is not consistent with chain MMR")]
    NewBlockInvalidChainRoot,
    #[error("received invalid note root")]
    NewBlockInvalidNoteRoot,
    #[error("received invalid nullifier root")]
    NewBlockInvalidNullifierRoot,
    #[error("new block `prev_hash` must match the chain's tip")]
    NewBlockInvalidPrevHash,
}

#[derive(Error, Debug)]
pub enum ApplyBlockError {
    // ERRORS WITH AUTOMATIC CONVERSIONS FROM NESTED ERROR TYPES
    // ---------------------------------------------------------------------------------------------
    #[error("database error")]
    DatabaseError(#[from] DatabaseError),
    #[error("I/O error")]
    IoError(#[from] io::Error),
    #[error("task join error")]
    TokioJoinError(#[from] tokio::task::JoinError),
    #[error("invalid block error")]
    InvalidBlockError(#[from] InvalidBlockError),

    // OTHER ERRORS
    // ---------------------------------------------------------------------------------------------
    #[error("block applying was cancelled because of closed channel on database side")]
    ClosedChannel(#[from] RecvError),
    #[error("concurrent write detected")]
    ConcurrentWrite,
    #[error("database doesn't have any block header data")]
    DbBlockHeaderEmpty,
    #[error("database update failed: {0}")]
    DbUpdateTaskFailed(String),
}

impl From<ApplyBlockError> for Status {
    fn from(err: ApplyBlockError) -> Self {
        match err {
            ApplyBlockError::InvalidBlockError(_) => Status::invalid_argument(err.to_string()),

            _ => Status::internal(err.to_string()),
        }
    }
}

#[derive(Error, Debug)]
pub enum GetBlockHeaderError {
    #[error("database error")]
    DatabaseError(#[from] DatabaseError),
    #[error("error retrieving the merkle proof for the block")]
    MmrError(#[from] MmrError),
}

#[derive(Error, Debug)]
pub enum GetBlockInputsError {
    #[error("account error")]
    AccountError(#[from] AccountError),
    #[error("database error")]
    DatabaseError(#[from] DatabaseError),
    #[error("database doesn't have any block header data")]
    DbBlockHeaderEmpty,
    #[error("failed to get MMR peaks for forest ({forest}): {error}")]
    FailedToGetMmrPeaksForForest { forest: usize, error: MmrError },
    #[error("chain MMR forest expected to be 1 less than latest header's block num. Chain MMR forest: {forest}, block num: {block_num}")]
    IncorrectChainMmrForestNumber { forest: usize, block_num: BlockNumber },
    #[error("note inclusion proof MMR error")]
    NoteInclusionMmr(#[from] MmrError),
}

impl From<GetNoteInclusionProofError> for GetBlockInputsError {
    fn from(value: GetNoteInclusionProofError) -> Self {
        match value {
            GetNoteInclusionProofError::DatabaseError(db_err) => db_err.into(),
            GetNoteInclusionProofError::MmrError(mmr_err) => Self::NoteInclusionMmr(mmr_err),
        }
    }
}

#[derive(Error, Debug)]
pub enum StateSyncError {
    #[error("database error")]
    DatabaseError(#[from] DatabaseError),
    #[error("block headers table is empty")]
    EmptyBlockHeadersTable,
    #[error("failed to build MMR delta")]
    FailedToBuildMmrDelta(#[from] MmrError),
}

#[derive(Error, Debug)]
pub enum NoteSyncError {
    #[error("database error")]
    DatabaseError(#[from] DatabaseError),
    #[error("block headers table is empty")]
    EmptyBlockHeadersTable,
    #[error("error retrieving the merkle proof for the block")]
    MmrError(#[from] MmrError),
}

#[derive(Error, Debug)]
pub enum GetNoteInclusionProofError {
    #[error("database error")]
    DatabaseError(#[from] DatabaseError),
    #[error("Mmr error")]
    MmrError(#[from] MmrError),
}
