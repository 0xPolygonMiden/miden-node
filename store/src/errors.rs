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

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Missing database connection: {0}")]
    MissingDbConnection(#[from] PoolError),
    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),
    #[error("SQLite error: {0}")]
    FromSqlError(#[from] FromSqlError),
    #[error("SQLite migration error: {0}")]
    SqliteMigrationError(#[from] rusqlite_migration::Error),
    #[error("Pool build error: {0}")]
    PoolBuildError(#[from] deadpool_sqlite::BuildError),
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("Prost decode error: {0}")]
    DecodeError(#[from] DecodeError),
    #[error("SQLite pool interaction task failed: {0}")]
    InteractionTaskError(#[from] InteractionTaskError),
    #[error("Genesis block error: {0}")]
    GenesisBlockError(#[from] GenesisError),
    #[error("State error: {0}")]
    StateError(Box<StateError>),
    #[error("Block database is empty")]
    BlockDbIsEmpty,
    #[error("Decoding nullifier from database failed: {0}")]
    NullifierDecodingError(DeserializationError),
    #[error("Block applying was broken because of closed channel on state side: {0}")]
    BlockApplyingBrokenBecauseOfClosedChannel(RecvError),
}

impl From<StateError> for DbError {
    fn from(value: StateError) -> Self {
        Self::StateError(Box::new(value))
    }
}

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

#[derive(Debug, Error)]
pub enum GenesisError {
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
    SelectBlockHeaderByBlockNumError(Box<DbError>),
}

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DbError),
    #[error("Digest error: {0:?}")]
    DigestError(#[from] ParseError),
    #[error("Database doesnt have any block header data")]
    DbBlockHeaderEmpty,
    #[error("Field `{field_name}` required to be filled in protobuf representation of {entity}")]
    MissingFieldInProtobufRepresentation {
        entity: &'static str,
        field_name: &'static str,
    },
    #[error("Failed to get MMR peaks for forest ({forest}): {error}")]
    FailedToGetMmrPeaksForForest { forest: usize, error: MmrError },
    #[error("Failed to get MMR delta: {0}")]
    FailedToGetMmrDelta(MmrError),
    #[error("Chain MMR forest expected to be 1 less than latest header's block num. Chain MMR forest: {forest}, block num: {block_num}")]
    IncorrectChainMmrForestNumber { forest: usize, block_num: u32 },
    #[error("Failed to create accounts tree: {0}")]
    FailedToCreateAccountsTree(MerkleError),
    #[error("Failed to create nullifiers tree: {0}")]
    FailedToCreateNullifiersTree(MerkleError),
}

#[derive(Error, Debug)]
pub enum ApplyBlockError {
    #[error("Parse error: {0}")]
    ParseError(#[from] ParseError),
    #[error("Database error: {0}")]
    DatabaseError(#[from] DbError),
    #[error("State error: {0}")]
    StateError(#[from] StateError),
    #[error("Concurrent write detected")]
    ConcurrentWrite,
    #[error("New block number must be 1 greater than the current block number")]
    NewBlockInvalidBlockNum,
    #[error("New block `prev_hash` must match the chain's tip")]
    NewBlockInvalidPrevHash,
    #[error("Duplicated nullifiers {0:?}")]
    DuplicatedNullifiers(Vec<RpoDigest>),
    #[error("New block chain root is not consistent with chain MMR")]
    NewBlockInvalidChainRoot,
    #[error("Received invalid account tree root")]
    NewBlockInvalidAccountRoot,
    #[error("Received invalid note root")]
    NewBlockInvalidNoteRoot,
    #[error("Unable to create proof for note: {0}")]
    UnableToCreateProofForNote(MerkleError),
    #[error("Block applying was broken because of closed channel on database side: {0}")]
    BlockApplyingBrokenBecauseOfClosedChannel(RecvError),
    #[error("Failed to create notes tree: {0}")]
    FailedToCreateNotesTree(MerkleError),
    #[error("Received invalid account id")]
    InvalidAccountId,
    #[error("Missing `note_hash`")]
    MissingNoteHash,
    #[error("Received invalid nullifier tree root")]
    NewBlockInvalidNullifierRoot,
}
