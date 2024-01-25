use hex::{FromHex, ToHex};

use crate::{digest::Digest, error::ParseError};

pub const DIGEST_DATA_SIZE: usize = 32;

impl ToHex for &Digest {
    fn encode_hex<T: std::iter::FromIterator<char>>(&self) -> T {
        (*self).encode_hex()
    }

    fn encode_hex_upper<T: std::iter::FromIterator<char>>(&self) -> T {
        (*self).encode_hex_upper()
    }
}

impl ToHex for Digest {
    fn encode_hex<T: std::iter::FromIterator<char>>(&self) -> T {
        let mut data: Vec<char> = Vec::with_capacity(DIGEST_DATA_SIZE);
        data.extend(format!("{:016x}", self.d0).chars());
        data.extend(format!("{:016x}", self.d1).chars());
        data.extend(format!("{:016x}", self.d2).chars());
        data.extend(format!("{:016x}", self.d3).chars());
        data.into_iter().collect()
    }

    fn encode_hex_upper<T: std::iter::FromIterator<char>>(&self) -> T {
        let mut data: Vec<char> = Vec::with_capacity(DIGEST_DATA_SIZE);
        data.extend(format!("{:016X}", self.d0).chars());
        data.extend(format!("{:016X}", self.d1).chars());
        data.extend(format!("{:016X}", self.d2).chars());
        data.extend(format!("{:016X}", self.d3).chars());
        data.into_iter().collect()
    }
}

impl FromHex for Digest {
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

                Ok(Digest { d0, d1, d2, d3 })
            },
        }
    }
}

#[cfg(test)]
mod test {
    use hex::{self, FromHex, ToHex};
    use proptest::prelude::*;

    use crate::digest::Digest;

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
