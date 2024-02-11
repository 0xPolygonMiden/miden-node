use std::fmt::{Debug, Display, Formatter};

use hex::{FromHex, ToHex};
use miden_objects::{
    notes::{NoteId, Nullifier},
    Digest, Felt, StarkField,
};

use crate::{errors::ParseError, generated::digest};

// CONSTANTS
// ================================================================================================

pub const DIGEST_DATA_SIZE: usize = 32;

// FORMATTING
// ================================================================================================

impl Display for digest::Digest {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str(&self.encode_hex::<String>())
    }
}

impl Debug for digest::Digest {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl ToHex for &digest::Digest {
    fn encode_hex<T: FromIterator<char>>(&self) -> T {
        (*self).encode_hex()
    }

    fn encode_hex_upper<T: FromIterator<char>>(&self) -> T {
        (*self).encode_hex_upper()
    }
}

impl ToHex for digest::Digest {
    fn encode_hex<T: FromIterator<char>>(&self) -> T {
        let mut data: Vec<char> = Vec::with_capacity(DIGEST_DATA_SIZE);
        data.extend(format!("{:016x}", self.d0).chars());
        data.extend(format!("{:016x}", self.d1).chars());
        data.extend(format!("{:016x}", self.d2).chars());
        data.extend(format!("{:016x}", self.d3).chars());
        data.into_iter().collect()
    }

    fn encode_hex_upper<T: FromIterator<char>>(&self) -> T {
        let mut data: Vec<char> = Vec::with_capacity(DIGEST_DATA_SIZE);
        data.extend(format!("{:016X}", self.d0).chars());
        data.extend(format!("{:016X}", self.d1).chars());
        data.extend(format!("{:016X}", self.d2).chars());
        data.extend(format!("{:016X}", self.d3).chars());
        data.into_iter().collect()
    }
}

impl FromHex for digest::Digest {
    type Error = ParseError;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        let data = hex::decode(hex)?;

        match data.len() {
            size if size < DIGEST_DATA_SIZE => Err(ParseError::InsufficientData {
                expected: DIGEST_DATA_SIZE,
                got: size,
            }),
            size if size > DIGEST_DATA_SIZE => Err(ParseError::TooMuchData {
                expected: DIGEST_DATA_SIZE,
                got: size,
            }),
            _ => {
                let d0 = u64::from_be_bytes(data[..8].try_into().unwrap());
                let d1 = u64::from_be_bytes(data[8..16].try_into().unwrap());
                let d2 = u64::from_be_bytes(data[16..24].try_into().unwrap());
                let d3 = u64::from_be_bytes(data[24..32].try_into().unwrap());

                Ok(digest::Digest { d0, d1, d2, d3 })
            },
        }
    }
}

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

#[cfg(test)]
mod test {
    use hex::{self, FromHex, ToHex};
    use proptest::prelude::*;

    use crate::generated::digest::Digest;

    #[test]
    fn test_hex_digest() {
        let digest = Digest {
            d0: 3488802789098113751,
            d1: 5271242459988994564,
            d2: 17816570245237064784,
            d3: 10910963388447438895,
        };
        let encoded: String = ToHex::encode_hex(&digest);
        let round_trip: Result<Digest, _> = FromHex::from_hex::<&[u8]>(encoded.as_ref());
        assert_eq!(digest, round_trip.unwrap());

        let digest = Digest {
            d0: 0,
            d1: 0,
            d2: 0,
            d3: 0,
        };
        let encoded: String = ToHex::encode_hex(&digest);
        let round_trip: Result<Digest, _> = FromHex::from_hex::<&[u8]>(encoded.as_ref());
        assert_eq!(digest, round_trip.unwrap());
    }

    proptest! {
        #[test]
        fn test_encode_decode(
            d0: u64,
            d1: u64,
            d2: u64,
            d3: u64,
        ) {
            let digest = Digest { d0, d1, d2, d3 };
            let encoded: String = ToHex::encode_hex(&digest);
            let round_trip: Result<Digest, _> = FromHex::from_hex::<&[u8]>(encoded.as_ref());
            assert_eq!(digest, round_trip.unwrap());
        }
    }
}
