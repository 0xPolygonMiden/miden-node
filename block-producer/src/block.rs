use miden_objects::{accounts::AccountId, BlockHeader, Digest};

pub struct Block {
    pub header: BlockHeader,
    pub updated_accounts: Vec<(AccountId, Digest)>,
    pub created_notes: Vec<Digest>,
    pub new_nullifiers: Vec<Digest>,

    // TODO:
    // - full states for updated public accounts
    // - full states for created public notes
    // - zk proof
}
