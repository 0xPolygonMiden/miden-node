use miden_crypto::{
    merkle::{MerklePath, TieredSmtProof},
    Felt, FieldElement, Word,
};
use miden_objects::Digest as RpoDigest;

use crate::{
    domain::{convert, try_convert},
    errors::ParseError,
    tsmt,
};

// INTO
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

// FROM
// ================================================================================================

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
