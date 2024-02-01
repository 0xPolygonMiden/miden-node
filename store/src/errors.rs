use miden_crypto::hash::rpo::RpoDigest;
use miden_node_proto::error::ParseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Concurrent write detected")]
    ConcurrentWrite,
    #[error("DB doesnt have any block header data")]
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
}
