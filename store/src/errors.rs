use miden_crypto::hash::rpo::RpoDigest;
use miden_node_proto::error::ParseError;

#[derive(Clone, Debug)]
pub enum StateError {
    ConcurrentWrite,
    DbBlockHeaderEmpty,
    DigestError(ParseError),
    DuplicatedNullifiers(Vec<RpoDigest>),
    InvalidAccountId,
    InvalidAccountRoot,
    InvalidChainRoot,
    InvalidNoteRoot,
    InvalidNullifierRoot,
    MissingAccountHash,
    MissingAccountId,
    MissingAccountRoot,
    MissingBatchRoot,
    MissingChainRoot,
    MissingNoteHash,
    MissingNoteRoot,
    MissingNullifierRoot,
    MissingPrevHash,
    MissingProofHash,
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
            StateError::InvalidAccountRoot => write!(f, "Received invalid account tree root"),
            StateError::InvalidChainRoot => write!(f, "Received invalid chain mmr hash"),
            StateError::InvalidNoteRoot => write!(f, "Received invalid note root"),
            StateError::InvalidNullifierRoot => write!(f, "Received invalid nullifier tree root"),
            StateError::MissingAccountHash => write!(f, "Missing account_hash"),
            StateError::MissingAccountId => write!(f, "Missing account_id"),
            StateError::MissingAccountRoot => write!(f, "Missing account root"),
            StateError::MissingBatchRoot => write!(f, "Missing batch root"),
            StateError::MissingChainRoot => write!(f, "Missing chain root"),
            StateError::MissingNoteHash => write!(f, "Missing note hash"),
            StateError::MissingNoteRoot => write!(f, "Missing note root"),
            StateError::MissingNullifierRoot => write!(f, "Missing nullifier root"),
            StateError::MissingPrevHash => write!(f, "Missing prev hash"),
            StateError::MissingProofHash => write!(f, "Missing proof hash"),
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
