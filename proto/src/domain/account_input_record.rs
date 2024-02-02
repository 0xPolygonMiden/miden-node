use miden_crypto::merkle::MerklePath;
use miden_objects::{accounts::AccountId, Digest};

#[derive(Clone, Debug)]
pub struct AccountInputRecord {
    pub account_id: AccountId,
    pub account_hash: Digest,
    pub proof: MerklePath,
}
