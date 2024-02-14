use miden_objects::{
    crypto::{hash::rpo::RpoDigest, merkle::SmtProof},
    notes::Nullifier,
};

use crate::{
    errors::{MissingFieldHelper, ParseError},
    generated::{digest::Digest, responses::NullifierBlockInputRecord},
};

// FROM NULLIFIER
// ================================================================================================

impl From<&Nullifier> for Digest {
    fn from(value: &Nullifier) -> Self {
        (*value).inner().into()
    }
}

impl From<Nullifier> for Digest {
    fn from(value: Nullifier) -> Self {
        value.inner().into()
    }
}

// INTO NULLIFIER
// ================================================================================================

impl TryFrom<Digest> for Nullifier {
    type Error = ParseError;

    fn try_from(value: Digest) -> Result<Self, Self::Error> {
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

impl TryFrom<NullifierBlockInputRecord> for NullifierWitness {
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

impl From<NullifierWitness> for NullifierBlockInputRecord {
    fn from(value: NullifierWitness) -> Self {
        Self {
            nullifier: Some(value.nullifier.into()),
            opening: Some(value.proof.into()),
        }
    }
}
