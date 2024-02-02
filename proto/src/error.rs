use miden_crypto::merkle::MmrError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ParseError {
    #[error("Hex error: {0}")]
    HexError(#[from] hex::FromHexError),
    #[error("Too much data, expected {expected}, got {got}")]
    TooMuchData { expected: usize, got: usize },
    #[error("Not enough data, expected {expected}, got {got}")]
    InsufficientData { expected: usize, got: usize },
    #[error("Tiered sparse merkle tree proof missing key")]
    MissingLeafKey,
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
}
