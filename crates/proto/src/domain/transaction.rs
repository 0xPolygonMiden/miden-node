use miden_objects::{crypto::hash::rpo::RpoDigest, transaction::TransactionId};

use crate::{
    errors::ConversionError,
    generated::{digest::Digest, transaction::TransactionInfo as TransactionInfoPb},
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

// INTO TRANSACTION ID
// ================================================================================================

impl TryFrom<Digest> for TransactionId {
    type Error = ConversionError;

    fn try_from(value: Digest) -> Result<Self, Self::Error> {
        let digest: RpoDigest = value.try_into()?;
        Ok(digest.into())
    }
}

#[derive(Debug, PartialEq)]
pub struct TransactionInfo {
    pub transaction_id: TransactionId,
    pub block_num: u32,
}

impl From<&TransactionInfo> for TransactionInfoPb {
    fn from(transaction_info: &TransactionInfo) -> Self {
        Self {
            transaction_id: Some(transaction_info.transaction_id.into()),
            block_num: transaction_info.block_num,
        }
    }
}

impl From<TransactionInfo> for TransactionInfoPb {
    fn from(transaction_info: TransactionInfo) -> Self {
        Self::from(&transaction_info)
    }
}
