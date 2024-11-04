use std::io;

use deadpool_sqlite::PoolError;
use miden_objects::{
    crypto::{
        hash::rpo::RpoDigest,
        merkle::{MerkleError, MmrError},
        utils::DeserializationError,
    },
    notes::Nullifier,
    transaction::OutputNote,
    AccountDeltaError, AccountError, BlockError, BlockHeader, NoteError,
};
use rusqlite::types::FromSqlError;
use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;
use tonic::Status;

use crate::types::{AccountId, BlockNumber};

// INTERNAL ERRORS
// =================================================================================================

#[derive(Debug, Error)]
pub enum NullifierTreeError {
    #[error("Merkle error: {0}")]
    MerkleError(#[from] MerkleError),
    #[error("Nullifier {nullifier} for block #{block_num} already exists in the nullifier tree")]
    NullifierAlreadyExists {
        nullifier: Nullifier,
        block_num: BlockNumber,
    },
}

// DATABASE ERRORS
// =================================================================================================

#[derive(Debug, Error)]
pub enum DatabaseError {
    // ERRORS WITH AUTOMATIC CONVERSIONS FROM NESTED ERROR TYPES
    // ---------------------------------------------------------------------------------------------
    #[error("Account error: {0}")]
    AccountError(#[from] AccountError),
    #[error("Account delta error: {0}")]
    AccountDeltaError(#[from] AccountDeltaError),
    #[error("Block error: {0}")]
    BlockError(#[from] BlockError),
    #[error("Closed channel: {0}")]
    ClosedChannel(#[from] RecvError),
    #[error("Deserialization of BLOB data from database failed: {0}")]
    DeserializationError(DeserializationError),
    #[error("Hex parsing error: {0}")]
    FromHexError(#[from] hex::FromHexError),
    #[error("SQLite error: {0}")]
    FromSqlError(#[from] FromSqlError),
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("Migration error: {0}")]
    MigrationError(#[from] rusqlite_migration::Error),
    #[error("Missing database connection: {0}")]
    MissingDbConnection(#[from] PoolError),
    #[error("Note error: {0}")]
    NoteError(#[from] NoteError),
    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),

    // OTHER ERRORS
    // ---------------------------------------------------------------------------------------------
    #[error("Account hashes mismatch (expected {expected}, but calculated is {calculated})")]
    AccountHashesMismatch {
        expected: RpoDigest,
        calculated: RpoDigest,
    },
    #[error("Account {0} not found in the database")]
    AccountNotFoundInDb(AccountId),
    #[error("Accounts {0:?} not found in the database")]
    AccountsNotFoundInDb(Vec<AccountId>),
    #[error("Account {0} is not on the chain")]
    AccountNotOnChain(AccountId),
    #[error("Block {0} not found in the database")]
    BlockNotFoundInDb(BlockNumber),
    #[error("SQLite pool interaction task failed: {0}")]
    InteractError(String),
    #[error("Invalid Felt: {0}")]
    InvalidFelt(String),
    #[error(
        "Unsupported database version. There is no migration chain from/to this version. \
        Remove all database files and try again."
    )]
    UnsupportedDatabaseVersion,
}

impl From<DeserializationError> for DatabaseError {
    fn from(value: DeserializationError) -> Self {
        Self::DeserializationError(value)
    }
}

impl From<DatabaseError> for Status {
    fn from(err: DatabaseError) -> Self {
        match err {
            DatabaseError::AccountNotFoundInDb(_)
            | DatabaseError::AccountsNotFoundInDb(_)
            | DatabaseError::AccountNotOnChain(_)
            | DatabaseError::BlockNotFoundInDb(_) => Status::not_found(err.to_string()),

            _ => Status::internal(err.to_string()),
        }
    }
}

// INITIALIZATION ERRORS
// =================================================================================================

#[derive(Error, Debug)]
pub enum StateInitializationError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Failed to create nullifier tree: {0}")]
    FailedToCreateNullifierTree(NullifierTreeError),
    #[error("Failed to create accounts tree: {0}")]
    FailedToCreateAccountsTree(MerkleError),
}

#[derive(Debug, Error)]
pub enum DatabaseSetupError {
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Genesis block error: {0}")]
    GenesisBlockError(#[from] GenesisError),
    #[error("Pool build error: {0}")]
    PoolBuildError(#[from] deadpool_sqlite::BuildError),
    #[error("SQLite migration error: {0}")]
    SqliteMigrationError(#[from] rusqlite_migration::Error),
}

#[derive(Debug, Error)]
pub enum GenesisError {
    // ERRORS WITH AUTOMATIC CONVERSIONS FROM NESTED ERROR TYPES
    // ---------------------------------------------------------------------------------------------
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Block error: {0}")]
    BlockError(#[from] BlockError),
    #[error("Merkle error: {0}")]
    MerkleError(#[from] MerkleError),

    // OTHER ERRORS
    // ---------------------------------------------------------------------------------------------
    #[error("Apply block failed: {0}")]
    ApplyBlockFailed(String),
    #[error("Failed to read genesis file \"{genesis_filepath}\": {error}")]
    FailedToReadGenesisFile {
        genesis_filepath: String,
        error: io::Error,
    },
    #[error("Block header in store doesn't match block header in genesis file. Expected {expected_genesis_header:?}, but store contained {block_header_in_store:?}")]
    GenesisBlockHeaderMismatch {
        expected_genesis_header: Box<BlockHeader>,
        block_header_in_store: Box<BlockHeader>,
    },
    #[error("Failed to deserialize genesis file: {0}")]
    GenesisFileDeserializationError(DeserializationError),
    #[error("Retrieving genesis block header failed: {0}")]
    SelectBlockHeaderByBlockNumError(Box<DatabaseError>),
}

// ENDPOINT ERRORS
// =================================================================================================
#[derive(Error, Debug)]
pub enum InvalidBlockError {
    #[error("Duplicated nullifiers {0:?}")]
    DuplicatedNullifiers(Vec<Nullifier>),
    #[error("Invalid output note type: {0:?}")]
    InvalidOutputNoteType(Box<OutputNote>),
    #[error("Invalid tx hash: expected {expected}, but got {actual}")]
    InvalidTxHash { expected: RpoDigest, actual: RpoDigest },
    #[error("Received invalid account tree root")]
    NewBlockInvalidAccountRoot,
    #[error("New block number must be 1 greater than the current block number")]
    NewBlockInvalidBlockNum,
    #[error("New block chain root is not consistent with chain MMR")]
    NewBlockInvalidChainRoot,
    #[error("Received invalid note root")]
    NewBlockInvalidNoteRoot,
    #[error("Received invalid nullifier root")]
    NewBlockInvalidNullifierRoot,
    #[error("New block `prev_hash` must match the chain's tip")]
    NewBlockInvalidPrevHash,
}

#[derive(Error, Debug)]
pub enum ApplyBlockError {
    // ERRORS WITH AUTOMATIC CONVERSIONS FROM NESTED ERROR TYPES
    // ---------------------------------------------------------------------------------------------
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("Task join error: {0}")]
    TokioJoinError(#[from] tokio::task::JoinError),
    #[error("Invalid block error: {0}")]
    InvalidBlockError(#[from] InvalidBlockError),

    // OTHER ERRORS
    // ---------------------------------------------------------------------------------------------
    #[error("Block applying was cancelled because of closed channel on database side: {0}")]
    ClosedChannel(RecvError),
    #[error("Concurrent write detected")]
    ConcurrentWrite,
    #[error("Database doesn't have any block header data")]
    DbBlockHeaderEmpty,
    #[error("Database update task failed: {0}")]
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
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Error retrieving the merkle proof for the block: {0}")]
    MmrError(#[from] MmrError),
}

#[derive(Error, Debug)]
pub enum GetBlockInputsError {
    #[error("Account error: {0}")]
    AccountError(#[from] AccountError),
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Database doesn't have any block header data")]
    DbBlockHeaderEmpty,
    #[error("Failed to get MMR peaks for forest ({forest}): {error}")]
    FailedToGetMmrPeaksForForest { forest: usize, error: MmrError },
    #[error("Chain MMR forest expected to be 1 less than latest header's block num. Chain MMR forest: {forest}, block num: {block_num}")]
    IncorrectChainMmrForestNumber { forest: usize, block_num: u32 },
    #[error("Note inclusion proof MMR error: {0}")]
    NoteInclusionMmr(MmrError),
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
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Block headers table is empty")]
    EmptyBlockHeadersTable,
    #[error("Failed to build MMR delta: {0}")]
    FailedToBuildMmrDelta(MmrError),
}

#[derive(Error, Debug)]
pub enum NoteSyncError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Block headers table is empty")]
    EmptyBlockHeadersTable,
    #[error("Error retrieving the merkle proof for the block: {0}")]
    MmrError(#[from] MmrError),
}

#[derive(Error, Debug)]
pub enum GetNoteInclusionProofError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Mmr error: {0}")]
    MmrError(#[from] MmrError),
}
