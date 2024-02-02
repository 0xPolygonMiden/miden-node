use miden_crypto::merkle::MerklePath;
use miden_objects::Digest;

use crate::{error, responses};

#[derive(Clone, Debug)]
pub struct NullifierInputRecord {
    pub nullifier: Digest,
    pub proof: MerklePath,
}

// FROM
// ================================================================================================

impl TryFrom<responses::NullifierBlockInputRecord> for NullifierInputRecord {
    type Error = error::ParseError;

    fn try_from(
        nullifier_input_record: responses::NullifierBlockInputRecord
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            nullifier: nullifier_input_record
                .nullifier
                .ok_or(error::ParseError::ProtobufMissingData)?
                .try_into()?,
            proof: nullifier_input_record
                .proof
                .ok_or(error::ParseError::ProtobufMissingData)?
                .try_into()?,
        })
    }
}
