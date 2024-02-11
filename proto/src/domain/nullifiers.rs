use miden_objects::{crypto::merkle::SmtProof, Digest};

use crate::{
    errors::{MissingFieldHelper, ParseError},
    generated::responses::NullifierBlockInputRecord,
};

// NULLIFIER INPUT RECORD
// ================================================================================================

#[derive(Clone, Debug)]
pub struct NullifierInputRecord {
    pub nullifier: Digest,
    pub proof: SmtProof,
}

impl TryFrom<NullifierBlockInputRecord> for NullifierInputRecord {
    type Error = ParseError;

    fn try_from(nullifier_input_record: NullifierBlockInputRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            nullifier: nullifier_input_record
                .nullifier
                .ok_or(NullifierBlockInputRecord::missing_field(stringify!(nullifier)))?
                .try_into()?,
            proof: nullifier_input_record
                .opening
                .ok_or(NullifierBlockInputRecord::missing_field(stringify!(opening)))?
                .try_into()?,
        })
    }
}

impl From<NullifierInputRecord> for NullifierBlockInputRecord {
    fn from(value: NullifierInputRecord) -> Self {
        Self {
            nullifier: Some(value.nullifier.into()),
            opening: Some(value.proof.into()),
        }
    }
}
