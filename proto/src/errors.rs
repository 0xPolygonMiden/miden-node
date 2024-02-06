use miden_crypto::merkle::{MmrError, SmtLeafError};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParseError {
    #[error("Hex error: {0}")]
    HexError(#[from] hex::FromHexError),
    #[error("Too much data, expected {expected}, got {got}")]
    TooMuchData { expected: usize, got: usize },
    #[error("Not enough data, expected {expected}, got {got}")]
    InsufficientData { expected: usize, got: usize },
    #[error("MmrPeaks error: {0}")]
    MmrPeaksError(MmrError),
    #[error("Number of MmrPeaks doesn't fit into memory")]
    TooManyMmrPeaks,
    #[error("Value is not in the range 0..MODULUS")]
    NotAValidFelt,
    #[error("Received TSMT proof is invalid")]
    InvalidProof,
    #[error("Protobuf message missing data")]
    ProtobufMissingData,
    #[error("smt leaf error: {0}")]
    SmtLeafError(#[from] SmtLeafError)
}
