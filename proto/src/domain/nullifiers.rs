use std::any::type_name;

use miden_crypto::{
    merkle::{MerklePath, TieredSmtProof},
    Felt, FieldElement, Word,
};
use miden_objects::{Digest, Digest as RpoDigest};

use crate::{
    domain::{convert, nullifier_value_to_blocknum, try_convert},
    errors::ParseError,
    generated::{responses, responses::NullifierBlockInputRecord, tsmt},
};

// NullifierLeaf
// ================================================================================================

impl From<(RpoDigest, Word)> for tsmt::NullifierLeaf {
    fn from(value: (RpoDigest, Word)) -> Self {
        let (key, value) = value;
        Self {
            key: Some(key.into()),
            block_num: nullifier_value_to_blocknum(value),
        }
    }
}

// NullifierProof
// ================================================================================================

impl From<TieredSmtProof> for tsmt::NullifierProof {
    fn from(value: TieredSmtProof) -> Self {
        let (path, entries) = value.into_parts();

        tsmt::NullifierProof {
            merkle_path: convert(path),
            leaves: convert(entries),
        }
    }
}

impl TryFrom<tsmt::NullifierProof> for TieredSmtProof {
    type Error = ParseError;

    fn try_from(value: tsmt::NullifierProof) -> Result<Self, Self::Error> {
        let path = MerklePath::new(try_convert(value.merkle_path)?);
        let entries = value
            .leaves
            .into_iter()
            .map(|leaf| {
                let key = leaf.key.ok_or(ParseError::MissingLeafKey)?.try_into()?;
                let value = [Felt::ZERO, Felt::ZERO, Felt::ZERO, Felt::from(leaf.block_num)];
                let result = (key, value);

                Ok(result)
            })
            .collect::<Result<Vec<(RpoDigest, Word)>, Self::Error>>()?;
        TieredSmtProof::new(path, entries).or(Err(ParseError::InvalidProof))
    }
}

// NullifierInputRecord
// ================================================================================================

#[derive(Clone, Debug)]
pub struct NullifierInputRecord {
    pub nullifier: Digest,
    pub proof: MerklePath,
}

impl TryFrom<responses::NullifierBlockInputRecord> for NullifierInputRecord {
    type Error = ParseError;

    fn try_from(
        nullifier_input_record: responses::NullifierBlockInputRecord
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            nullifier: nullifier_input_record
                .nullifier
                .ok_or(ParseError::MissingFieldInProtobufRepresentation {
                    entity: type_name::<NullifierBlockInputRecord>(),
                    field_name: stringify!(nullifier),
                })?
                .try_into()?,
            proof: nullifier_input_record
                .proof
                .ok_or(ParseError::MissingFieldInProtobufRepresentation {
                    entity: type_name::<NullifierBlockInputRecord>(),
                    field_name: stringify!(proof),
                })?
                .try_into()?,
        })
    }
}
