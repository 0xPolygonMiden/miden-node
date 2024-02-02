use miden_crypto::merkle::MmrDelta;
use miden_objects::Digest as RpoDigest;

use crate::{digest, error, mmr};

// INTO
// ================================================================================================

impl From<MmrDelta> for mmr::MmrDelta {
    fn from(value: MmrDelta) -> Self {
        let data: Vec<digest::Digest> = value.data.into_iter().map(|v| v.into()).collect();

        mmr::MmrDelta {
            forest: value.forest as u64,
            data,
        }
    }
}

// FROM
// ================================================================================================

impl TryFrom<mmr::MmrDelta> for MmrDelta {
    type Error = error::ParseError;

    fn try_from(value: mmr::MmrDelta) -> Result<Self, Self::Error> {
        let data: Result<Vec<RpoDigest>, error::ParseError> =
            value.data.into_iter().map(|v| v.try_into()).collect();

        Ok(MmrDelta {
            forest: value.forest as usize,
            data: data?,
        })
    }
}
