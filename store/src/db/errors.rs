use std::io;

use deadpool_sqlite::PoolError;
use miden_crypto::{merkle::MerkleError, utils::DeserializationError};
use miden_node_proto::block_header::BlockHeader;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenesisBlockError {
    #[error("apply block failed: {0}")]
    ApplyBlockFailed(String),
    #[error("failed to read genesis file \"{genesis_filepath}\": {error}")]
    FailedToReadGenesisFile {
        genesis_filepath: String,
        error: io::Error,
    },
    #[error("failed to deserialize genesis file: {0}")]
    GenesisFileDeserializationError(DeserializationError),
    #[error("block header in store doesn't match block header in genesis file. Expected {expected_genesis_header:?}, but store contained {block_header_in_store:?}")]
    GenesisBlockHeaderMismatch {
        expected_genesis_header: Box<BlockHeader>,
        block_header_in_store: Box<BlockHeader>,
    },
    #[error("malconstructed genesis state: {0}")]
    MalconstructedGenesisState(#[from] MerkleError),
    #[error("missing db connection: {0}")]
    MissingDbConnection(#[from] PoolError),
    #[error("retrieving genesis block header failed: {0}")]
    SelectBlockHeaderByBlockNumError(String),
}
