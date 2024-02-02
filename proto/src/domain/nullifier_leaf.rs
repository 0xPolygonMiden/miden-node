use miden_crypto::Word;
use miden_objects::Digest as RpoDigest;

use crate::{domain::nullifier_value_to_blocknum, tsmt};

// INTO
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
