use miden_crypto::hash::rpo::RpoDigest;
use miden_node_proto::error::ParseError;

#[derive(Clone, Debug)]
pub enum StateError {
    ConcurrentWrite,
    DbBlockHeaderEmpty,
    DigestError(ParseError),
    DuplicatedNullifiers(Vec<RpoDigest>),
    InvalidAccountId,
    MissingAccountHash,
    MissingAccountId,
    MissingNoteHash,
    NewBlockInvalidAccountRoot,
    NewBlockInvalidBlockNum,
    NewBlockInvalidChainRoot,
    NewBlockInvalidNoteRoot,
    NewBlockInvalidNullifierRoot,
    NewBlockInvalidPrevHash,
    NoteMissingHash,
    NoteMissingMerklePath,
}

impl std::error::Error for StateError {}

impl std::fmt::Display for StateError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            StateError::ConcurrentWrite => write!(f, "Concurrent write detected"),
            StateError::DbBlockHeaderEmpty => write!(f, "DB doesnt have any block header data"),
            StateError::DigestError(digest_error) => write!(f, "{:?}", digest_error),
            StateError::DuplicatedNullifiers(nullifiers) => {
                write!(f, "Duplicated nullifiers {:?}", nullifiers)
            },
            StateError::InvalidAccountId => write!(f, "Received invalid account id"),
            StateError::MissingAccountHash => write!(f, "Missing account_hash"),
            StateError::MissingAccountId => write!(f, "Missing account_id"),
            StateError::MissingNoteHash => write!(f, "Missing note hash"),
            StateError::NewBlockInvalidAccountRoot => {
                write!(f, "Received invalid account tree root")
            },
            StateError::NewBlockInvalidBlockNum => {
                write!(f, "New block number must be 1 greater than the current block number")
            },
            StateError::NewBlockInvalidChainRoot => {
                write!(f, "New block chain root is not consistent with chain MMR")
            },
            StateError::NewBlockInvalidNoteRoot => write!(f, "Received invalid note root"),
            StateError::NewBlockInvalidNullifierRoot => {
                write!(f, "Received invalid nullifier tree root")
            },
            StateError::NewBlockInvalidPrevHash => {
                write!(f, "New block prev_hash must match the chain's tip")
            },
            StateError::NoteMissingHash => write!(f, "Note message is missing the note's hash"),
            StateError::NoteMissingMerklePath => {
                write!(f, "Note message is missing the merkle path")
            },
        }
    }
}
