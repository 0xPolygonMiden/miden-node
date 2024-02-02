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
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("Pool build error: {0}")]
    PoolBuildError(#[from] deadpool_sqlite::BuildError),
    #[error("Prost decode error: {0}")]
    DecodeError(#[from] DecodeError),
    #[error("SQLite pool interaction task failed: {0}")]
    InteractionTaskError(#[from] InteractionTaskError),
    #[error("Genesis block error: {0}")]
    GenesisBlockError(#[from] GenesisBlockError),
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
    #[error("Select block headers task failed: {0}")]
    SelectBlockHeadersTaskFailed(String),
    #[error("Get nullifiers task failed: {0}")]
    GetNullifiersTaskFailed(String),
    #[error("Get notes task failed: {0}")]
    GetNotesTaskFailed(String),
    #[error("Get accounts task failed: {0}")]
    GetAccountsTaskFailed(String),
    #[error("Get block header task failed: {0}")]
    GetBlockHeaderTaskFailed(String),
    #[error("Get block headers task failed: {0}")]
    GetBlockHeadersTaskFailed(String),
    #[error("Get account hashes task failed: {0}")]
    GetAccountHashesTaskFailed(String),
    #[error("Apply block task failed: {0}")]
    ApplyBlockTaskFailed(String),
}

#[derive(Debug, Error)]
pub enum GenesisBlockError {
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
    #[error("Concurrent write detected")]
    ConcurrentWrite,
    #[error("Database doesnt have any block header data")]
    DbBlockHeaderEmpty,
    #[error("Digest error: {0:?}")]
    DigestError(#[from] ParseError),
    #[error("Duplicated nullifiers {0:?}")]
    DuplicatedNullifiers(Vec<RpoDigest>),
    #[error("Received invalid account id")]
    InvalidAccountId,
    #[error("Missing `account_hash`")]
    MissingAccountHash,
    #[error("Missing `account_id`")]
    MissingAccountId,
    #[error("Missing `note_hash`")]
    MissingNoteHash,
    #[error("Received invalid account tree root")]
    NewBlockInvalidAccountRoot,
    #[error("New block number must be 1 greater than the current block number")]
    NewBlockInvalidBlockNum,
    #[error("New block chain root is not consistent with chain MMR")]
    NewBlockInvalidChainRoot,
    #[error("Received invalid note root")]
    NewBlockInvalidNoteRoot,
    #[error("Received invalid nullifier tree root")]
    NewBlockInvalidNullifierRoot,
    #[error("New block `prev_hash` must match the chain's tip")]
    NewBlockInvalidPrevHash,
    #[error("Note message is missing the note's hash")]
    NoteMissingHash,
    #[error("Note message is missing the merkle path")]
    NoteMissingMerklePath,
    #[error("Failed to get MMR peaks for forest ({forest}): {error}")]
    FailedToGetMmrPeaksForForest { forest: usize, error: MmrError },
    #[error("Failed to get MMR delta: {0}")]
    FailedToGetMmrDelta(MmrError),
    #[error("Chain MMR forest expected to be 1 less than latest header's block num. Chain MMR forest: {forest}, block num: {block_num}")]
    IncorrectChainMmrForestNumber { forest: usize, block_num: u32 },
    #[error("Unable to create proof for note: {0}")]
    UnableToCreateProofForNote(MerkleError),
    #[error("Failed to create accounts tree: {0}")]
    FailedToCreateAccountsTree(MerkleError),
    #[error("Failed to create nullifiers tree: {0}")]
    FailedToCreateNullifiersTree(MerkleError),
    #[error("Failed to create notes tree: {0}")]
    FailedToCreateNotesTree(MerkleError),
    #[error("Block applying was broken because of closed channel on database side: {0}")]
    BlockApplyingBrokenBecauseOfClosedChannel(RecvError),
    #[error("Database error: {0}")]
    DatabaseError(#[from] DbError),
}
