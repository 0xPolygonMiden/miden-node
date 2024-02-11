use miden_objects::{crypto::merkle::MmrDelta, Digest};

use crate::{errors::ParseError, generated};

// INTO
// ================================================================================================

impl From<MmrDelta> for generated::mmr::MmrDelta {
    fn from(value: MmrDelta) -> Self {
        let data: Vec<generated::digest::Digest> =
            value.data.into_iter().map(|v| v.into()).collect();

        generated::mmr::MmrDelta {
            forest: value.forest as u64,
            data,
        }
    }
}

// FROM
// ================================================================================================

impl TryFrom<generated::mmr::MmrDelta> for MmrDelta {
    type Error = ParseError;

    fn try_from(value: generated::mmr::MmrDelta) -> Result<Self, Self::Error> {
        let data: Result<Vec<Digest>, ParseError> =
            value.data.into_iter().map(|v| v.try_into()).collect();

        Ok(MmrDelta {
            forest: value.forest as usize,
            data: data?,
        })
    }
}
