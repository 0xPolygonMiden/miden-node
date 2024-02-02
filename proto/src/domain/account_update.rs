use miden_objects::{accounts::AccountId, Digest as RpoDigest};

use crate::requests;

// INTO
// ================================================================================================

impl From<(AccountId, RpoDigest)> for requests::AccountUpdate {
    fn from((account_id, account_hash): (AccountId, RpoDigest)) -> Self {
        Self {
            account_id: Some(account_id.into()),
            account_hash: Some(account_hash.into()),
        }
    }
}
