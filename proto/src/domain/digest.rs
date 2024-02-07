use miden_crypto::{Felt, StarkField};
use miden_objects::{
    notes::{NoteId, Nullifier},
    Digest,
};

use crate::{digest, errors::ParseError};

// INTO
// ================================================================================================

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

impl From<&[u64; 4]> for digest::Digest {
    fn from(value: &[u64; 4]) -> Self {
        (*value).into()
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

impl From<&[Felt; 4]> for digest::Digest {
    fn from(value: &[Felt; 4]) -> Self {
        (*value).into()
    }
}

impl From<Digest> for digest::Digest {
    fn from(value: Digest) -> Self {
        Self {
            d0: value[0].as_int(),
            d1: value[1].as_int(),
            d2: value[2].as_int(),
            d3: value[3].as_int(),
        }
    }
}

impl From<&Digest> for digest::Digest {
    fn from(value: &Digest) -> Self {
        (*value).into()
    }
}

impl From<&Nullifier> for digest::Digest {
    fn from(value: &Nullifier) -> Self {
        (*value).inner().into()
    }
}

impl From<Nullifier> for digest::Digest {
    fn from(value: Nullifier) -> Self {
        value.inner().into()
    }
}

impl From<&NoteId> for digest::Digest {
    fn from(value: &NoteId) -> Self {
        (*value).inner().into()
    }
}

impl From<NoteId> for digest::Digest {
    fn from(value: NoteId) -> Self {
        value.inner().into()
    }
}

// FROM DIGEST
// ================================================================================================

impl From<digest::Digest> for [u64; 4] {
    fn from(value: digest::Digest) -> Self {
        [value.d0, value.d1, value.d2, value.d3]
    }
}

impl TryFrom<digest::Digest> for [Felt; 4] {
    type Error = ParseError;

    fn try_from(value: digest::Digest) -> Result<Self, Self::Error> {
        if ![value.d0, value.d1, value.d2, value.d3]
            .iter()
            .all(|v| *v < <Felt as StarkField>::MODULUS)
        {
            Err(ParseError::NotAValidFelt)
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

impl TryFrom<digest::Digest> for Digest {
    type Error = ParseError;

    fn try_from(value: digest::Digest) -> Result<Self, Self::Error> {
        Ok(Self::new(value.try_into()?))
    }
}

impl TryFrom<&digest::Digest> for [Felt; 4] {
    type Error = ParseError;

    fn try_from(value: &digest::Digest) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl TryFrom<&digest::Digest> for Digest {
    type Error = ParseError;

    fn try_from(value: &digest::Digest) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}
