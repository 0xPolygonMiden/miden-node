use miden_crypto::{
    hash::rpo::RpoDigest,
    merkle::{MerkleError, MmrError},
};
use miden_node_proto::error::ParseError;
use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;

use crate::db::errors::DbError;

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
