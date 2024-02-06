use std::io;

use deadpool_sqlite::PoolError;
use miden_crypto::{
    hash::rpo::RpoDigest,
    merkle::{MerkleError, MmrError},
    utils::DeserializationError,
};
use miden_node_proto::{block_header::BlockHeader, errors::ParseError};
use prost::DecodeError;
use rusqlite::types::FromSqlError;
use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;

// Conversion errors
// =================================================================================================

#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("Parse error: {0}")]
    ParseError(#[from] ParseError),
    #[error("Field `{field_name}` required to be filled in protobuf representation of {entity}")]
    MissingFieldInProtobufRepresentation {
        entity: &'static str,
        field_name: &'static str,
    },
}

// Error type from Deadpool's `interaction` task errors
// =================================================================================================

#[derive(Debug, Error)]
pub enum InteractionTaskError {
    #[error("Migration task failed: {0}")]
    MigrationTaskFailed(String),
    #[error("Select nullifiers task failed: {0}")]
    SelectNullifiersTaskFailed(String),
    #[error("Select notes task failed: {0}")]
    SelectNotesTaskFailed(String),
    #[error("Select accounts task failed: {0}")]
    SelectAccountsTaskFailed(String),
    #[error("Select block header task failed: {0}")]
    SelectBlockHeaderTaskFailed(String),
    #[error("Select block headers task failed: {0}")]
    SelectBlockHeadersTaskFailed(String),
    #[error("Select account hashes task failed: {0}")]
    SelectAccountHashesTaskFailed(String),
    #[error("Get state sync task failed: {0}")]
    GetStateSyncTaskFailed(String),
    #[error("Apply block task failed: {0}")]
    ApplyBlockTaskFailed(String),
}

// Database errors
// =================================================================================================

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Missing database connection: {0}")]
    MissingDbConnection(#[from] PoolError),
    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),
    #[error("SQLite error: {0}")]
    FromSqlError(#[from] FromSqlError),
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("Prost decode error: {0}")]
    DecodeError(#[from] DecodeError),
    #[error("SQLite pool interaction task failed: {0}")]
    InteractionTaskError(#[from] InteractionTaskError),
    #[error("Conversion error: {0}")]
    ConversionError(#[from] ConversionError),
    #[error("Decoding nullifier from database failed: {0}")]
    NullifierDecodingError(DeserializationError),
    #[error("Block applying was broken because of closed channel on state side: {0}")]
    ApplyBlockFailedClosedChannel(RecvError),
}

#[derive(Debug, Error)]
pub enum DatabaseSetupError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Genesis block error: {0}")]
    GenesisBlockError(#[from] GenesisError),
    #[error("Pool build error: {0}")]
    PoolBuildError(#[from] deadpool_sqlite::BuildError),
    #[error("SQLite migration error: {0}")]
    SqliteMigrationError(#[from] rusqlite_migration::Error),
}

// Genesis errors
// =================================================================================================

#[derive(Debug, Error)]
pub enum GenesisError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Apply block failed: {0}")]
    ApplyBlockFailed(String),
    #[error("Failed to read genesis file \"{genesis_filepath}\": {error}")]
    FailedToReadGenesisFile {
        genesis_filepath: String,
        error: io::Error,
    },
    #[error("Failed to deserialize genesis file: {0}")]
    GenesisFileDeserializationError(DeserializationError),
    #[error("Block header in store doesn't match block header in genesis file. Expected {expected_genesis_header:?}, but store contained {block_header_in_store:?}")]
    GenesisBlockHeaderMismatch {
        expected_genesis_header: Box<BlockHeader>,
        block_header_in_store: Box<BlockHeader>,
    },
    #[error("Malconstructed genesis state: {0}")]
    MalconstructedGenesisState(MerkleError),
    #[error("Retrieving genesis block header failed: {0}")]
    SelectBlockHeaderByBlockNumError(Box<DatabaseError>),
}

// Get block inputs errors
// =================================================================================================

#[derive(Error, Debug)]
pub enum GetBlockInputsError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Database doesnt have any block header data")]
    DbBlockHeaderEmpty,
    #[error("Failed to get MMR peaks for forest ({forest}): {error}")]
    FailedToGetMmrPeaksForForest { forest: usize, error: MmrError },
    #[error("Chain MMR forest expected to be 1 less than latest header's block num. Chain MMR forest: {forest}, block num: {block_num}")]
    IncorrectChainMmrForestNumber { forest: usize, block_num: u32 },
}

// State initialization errors
// =================================================================================================

#[derive(Error, Debug)]
pub enum StateInitializationError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Conversion error: {0}")]
    ConversionError(#[from] ConversionError),
    #[error("Failed to create nullifiers tree: {0}")]
    FailedToCreateNullifiersTree(MerkleError),
    #[error("Failed to create accounts tree: {0}")]
    FailedToCreateAccountsTree(MerkleError),
    #[error("Failed to create chain MMR: {0}")]
    FailedToCreateChainMmr(ParseError),
}

// State synchronization errors
// =================================================================================================

#[derive(Error, Debug)]
pub enum StateSyncError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Block headers table is empty")]
    EmptyBlockHeadersTable,
    #[error("Failed to build MMR delta: {0}")]
    FailedToBuildMmrDelta(MmrError),
}

// Block applying errors
// =================================================================================================

#[derive(Error, Debug)]
pub enum ApplyBlockError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Conversion error: {0}")]
    ConversionError(#[from] ConversionError),
    #[error("Concurrent write detected")]
    ConcurrentWrite,
    #[error("New block number must be 1 greater than the current block number")]
    NewBlockInvalidBlockNum,
    #[error("New block `prev_hash` must match the chain's tip")]
    NewBlockInvalidPrevHash,
    #[error("New block chain root is not consistent with chain MMR")]
    NewBlockInvalidChainRoot,
    #[error("Received invalid account tree root")]
    NewBlockInvalidAccountRoot,
    #[error("Received invalid note root")]
    NewBlockInvalidNoteRoot,
    #[error("Duplicated nullifiers {0:?}")]
    DuplicatedNullifiers(Vec<RpoDigest>),
    #[error("Unable to create proof for note: {0}")]
    UnableToCreateProofForNote(MerkleError),
    #[error("Block applying was broken because of closed channel on database side: {0}")]
    BlockApplyingBrokenBecauseOfClosedChannel(RecvError),
    #[error("Failed to create notes tree: {0}")]
    FailedToCreateNotesTree(MerkleError),
    #[error("Received invalid account id")]
    InvalidAccountId,
    #[error("Database doesnt have any block header data")]
    DbBlockHeaderEmpty,
    #[error("Failed to get MMR peaks for forest ({forest}): {error}")]
    FailedToGetMmrPeaksForForest { forest: usize, error: MmrError },
}

impl From<ParseError> for ApplyBlockError {
    fn from(err: ParseError) -> Self {
        ApplyBlockError::ConversionError(err.into())
    }
}
