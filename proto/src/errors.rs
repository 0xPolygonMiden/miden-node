use std::any::type_name;

use miden_crypto::merkle::MmrError;
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
    #[error("Field `{field_name}` required to be filled in protobuf representation of {entity}")]
    MissingFieldInProtobufRepresentation {
        entity: &'static str,
        field_name: &'static str,
    },
}

pub trait MissingFieldHelper {
    fn missing_field(field_name: &'static str) -> ParseError;
}

impl<T> MissingFieldHelper for T {
    fn missing_field(field_name: &'static str) -> ParseError {
        ParseError::MissingFieldInProtobufRepresentation {
            entity: type_name::<T>(),
            field_name,
        }
    }
}
