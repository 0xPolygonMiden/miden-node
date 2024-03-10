use std::any::type_name;

use miden_objects::{
    crypto::merkle::{SmtLeafError, SmtProofError},
    utils::DeserializationError,
    AccountDeltaError, AssetError, AssetVaultError,
};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ConversionError {
    #[error("Hex error: {0}")]
    HexError(#[from] hex::FromHexError),
    #[error("SMT leaf error: {0}")]
    SmtLeafError(#[from] SmtLeafError),
    #[error("SMT proof error: {0}")]
    SmtProofError(#[from] SmtProofError),
    #[error("Account delta error: {0}")]
    AccountDeltaError(#[from] AccountDeltaError),
    #[error("Asset error: {0}")]
    AssetError(#[from] AssetError),
    #[error("Asset vault error: {0}")]
    AssetVaultError(#[from] AssetVaultError),
    #[error("Deserialization error: {0}")]
    DeserializationError(DeserializationError),
    #[error("Too much data, expected {expected}, got {got}")]
    TooMuchData { expected: usize, got: usize },
    #[error("Not enough data, expected {expected}, got {got}")]
    InsufficientData { expected: usize, got: usize },
    #[error("Number of MmrPeaks doesn't fit into memory")]
    TooManyMmrPeaks,
    #[error("Value is not in the range 0..MODULUS")]
    NotAValidFelt,
    #[error("Field `{field_name}` required to be filled in protobuf representation of {entity}")]
    MissingFieldInProtobufRepresentation {
        entity: &'static str,
        field_name: &'static str,
    },
}

impl From<DeserializationError> for ConversionError {
    fn from(value: DeserializationError) -> Self {
        Self::DeserializationError(value)
    }
}

pub trait MissingFieldHelper {
    fn missing_field(field_name: &'static str) -> ConversionError;
}

impl<T: prost::Message> MissingFieldHelper for T {
    fn missing_field(field_name: &'static str) -> ConversionError {
        ConversionError::MissingFieldInProtobufRepresentation {
            entity: type_name::<T>(),
            field_name,
        }
    }
}
