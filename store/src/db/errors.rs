use std::io;

use deadpool_sqlite::PoolError;
use miden_crypto::{merkle::MerkleError, utils::DeserializationError};
use miden_node_proto::block_header::BlockHeader;
use prost::DecodeError;
use rusqlite::types::FromSqlError;
use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;

use crate::errors::StateError;

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
