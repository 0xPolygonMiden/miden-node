use miden_crypto::merkle::MerklePath;

use crate::{digest, errors::ParseError, merkle};

// INTO
// ================================================================================================

impl From<MerklePath> for merkle::MerklePath {
    fn from(value: MerklePath) -> Self {
        let siblings: Vec<digest::Digest> = value.nodes().iter().map(|v| (*v).into()).collect();
        merkle::MerklePath { siblings }
    }
}

// FROM
// ================================================================================================

impl TryFrom<merkle::MerklePath> for MerklePath {
    type Error = ParseError;

    fn try_from(merkle_path: merkle::MerklePath) -> Result<Self, Self::Error> {
        merkle_path.siblings.into_iter().map(|v| v.try_into()).collect()
    }
}
