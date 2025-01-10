use miden_objects::{
    crypto::{hash::rpo::RpoDigest, merkle::SmtProof},
    notes::Nullifier,
};

use crate::{
    errors::{ConversionError, MissingFieldHelper},
    generated as proto,
};

// FROM NULLIFIER
// ================================================================================================

impl From<&Nullifier> for proto::digest::Digest {
    fn from(value: &Nullifier) -> Self {
        (*value).inner().into()
    }
}

impl From<Nullifier> for proto::digest::Digest {
    fn from(value: Nullifier) -> Self {
        value.inner().into()
    }
}

// INTO NULLIFIER
// ================================================================================================

impl TryFrom<proto::digest::Digest> for Nullifier {
    type Error = ConversionError;

    fn try_from(value: proto::digest::Digest) -> Result<Self, Self::Error> {
        let digest: RpoDigest = value.try_into()?;
        Ok(digest.into())
    }
}

// NULLIFIER INPUT RECORD
// ================================================================================================

#[derive(Clone, Debug)]
pub struct NullifierWitness {
    pub nullifier: Nullifier,
    pub proof: SmtProof,
}

impl TryFrom<proto::responses::NullifierBlockInputRecord> for NullifierWitness {
    type Error = ConversionError;

    fn try_from(
        nullifier_input_record: proto::responses::NullifierBlockInputRecord,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            nullifier: nullifier_input_record
                .nullifier
                .ok_or(proto::responses::NullifierBlockInputRecord::missing_field(stringify!(
                    nullifier
                )))?
                .try_into()?,
            proof: nullifier_input_record
                .opening
                .ok_or(proto::responses::NullifierBlockInputRecord::missing_field(stringify!(
                    opening
                )))?
                .try_into()?,
        })
    }
}

impl From<NullifierWitness> for proto::responses::NullifierBlockInputRecord {
    fn from(value: NullifierWitness) -> Self {
        Self {
            nullifier: Some(value.nullifier.into()),
            opening: Some(value.proof.into()),
        }
    }
}
