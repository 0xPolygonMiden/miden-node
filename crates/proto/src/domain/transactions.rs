use miden_objects::{crypto::hash::rpo::RpoDigest, transaction::TransactionId};

use crate::{
    errors::ConversionError,
    generated::{digest::Digest, transaction::TransactionId as TransactionIdPb},
};

// FROM TRANSACTION ID
// ================================================================================================

impl From<&TransactionId> for Digest {
    fn from(value: &TransactionId) -> Self {
        (*value).inner().into()
    }
}

impl From<TransactionId> for Digest {
    fn from(value: TransactionId) -> Self {
        value.inner().into()
    }
}

impl From<&TransactionId> for TransactionIdPb {
    fn from(value: &TransactionId) -> Self {
        TransactionIdPb { id: Some(value.into()) }
    }
}

impl From<TransactionId> for TransactionIdPb {
    fn from(value: TransactionId) -> Self {
        (&value).into()
    }
}

// INTO TRANSACTION ID
// ================================================================================================

impl TryFrom<Digest> for TransactionId {
    type Error = ConversionError;

    fn try_from(value: Digest) -> Result<Self, Self::Error> {
        let digest: RpoDigest = value.try_into()?;
        Ok(digest.into())
    }
}

impl TryFrom<TransactionIdPb> for TransactionId {
    type Error = ConversionError;

    fn try_from(value: TransactionIdPb) -> Result<Self, Self::Error> {
        value
            .id
            .ok_or(ConversionError::MissingFieldInProtobufRepresentation {
                entity: "TransactionId",
                field_name: "id",
            })?
            .try_into()
    }
}
