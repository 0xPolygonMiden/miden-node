use miden_objects::{crypto::hash::rpo::RpoDigest, transaction::TransactionId};

use crate::{errors::ConversionError, generated::digest::Digest};

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

// INTO TRANSACTION ID
// ================================================================================================

impl TryFrom<Digest> for TransactionId {
    type Error = ConversionError;

    fn try_from(value: Digest) -> Result<Self, Self::Error> {
        let digest: RpoDigest = value.try_into()?;
        Ok(digest.into())
    }
}
