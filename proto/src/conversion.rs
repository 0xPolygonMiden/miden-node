use crate::digest;
use crate::error;
use crate::tsmt;
use miden_crypto::hash::rpo::RpoDigest;
use miden_crypto::merkle::MerklePath;
use miden_crypto::merkle::TieredSmtProof;
use miden_crypto::Felt;
use miden_crypto::FieldElement;
use miden_crypto::StarkField;
use miden_crypto::Word;

impl From<[u64; 4]> for digest::Digest {
    fn from(value: [u64; 4]) -> Self {
        Self {
            d0: value[0],
            d1: value[1],
            d2: value[2],
            d3: value[3],
        }
    }
}

impl From<[Felt; 4]> for digest::Digest {
    fn from(value: [Felt; 4]) -> Self {
        Self {
            d0: value[0].as_int(),
            d1: value[1].as_int(),
            d2: value[2].as_int(),
            d3: value[3].as_int(),
        }
    }
}

impl From<RpoDigest> for digest::Digest {
    fn from(value: RpoDigest) -> Self {
        Self {
            d0: value[0].as_int(),
            d1: value[1].as_int(),
            d2: value[2].as_int(),
            d3: value[3].as_int(),
        }
    }
}

impl From<digest::Digest> for [u64; 4] {
    fn from(value: digest::Digest) -> Self {
        [value.d0, value.d1, value.d2, value.d3]
    }
}

impl TryFrom<tsmt::NullifierProof> for TieredSmtProof {
    type Error = error::ParseError;

    fn try_from(value: tsmt::NullifierProof) -> Result<Self, Self::Error> {
        let path = MerklePath::new(
            value
                .merkle_path
                .into_iter()
                .map(|v| v.try_into())
                .collect::<Result<_, Self::Error>>()?,
        );
        let entries = value
            .leaves
            .into_iter()
            .map(|leaf| {
                let key = leaf.key.ok_or(error::ParseError::MissingLeafKey)?.try_into()?;
                let value = [Felt::ZERO, Felt::ZERO, Felt::ZERO, Felt::new(leaf.value)];
                let result = (key, value);

                Ok(result)
            })
            .collect::<Result<Vec<(RpoDigest, Word)>, Self::Error>>()?;
        TieredSmtProof::new(path, entries).or(Err(error::ParseError::InvalidProof))
    }
}

impl TryFrom<digest::Digest> for [Felt; 4] {
    type Error = error::ParseError;

    fn try_from(value: digest::Digest) -> Result<Self, Self::Error> {
        if ![value.d0, value.d1, value.d2, value.d3]
            .iter()
            .all(|v| *v < <Felt as StarkField>::MODULUS)
        {
            Err(error::ParseError::NotAValidFelt)
        } else {
            Ok([
                Felt::new(value.d0),
                Felt::new(value.d1),
                Felt::new(value.d2),
                Felt::new(value.d3),
            ])
        }
    }
}

impl TryFrom<digest::Digest> for RpoDigest {
    type Error = error::ParseError;

    fn try_from(value: digest::Digest) -> Result<Self, Self::Error> {
        Ok(Self::new(value.try_into()?))
    }
}

impl TryFrom<&digest::Digest> for [Felt; 4] {
    type Error = error::ParseError;

    fn try_from(value: &digest::Digest) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl TryFrom<&digest::Digest> for RpoDigest {
    type Error = error::ParseError;

    fn try_from(value: &digest::Digest) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}
