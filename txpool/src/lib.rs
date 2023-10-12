use miden_objects::{transaction::ProvenTransaction, BlockHeader, Digest, accounts::AccountId};

pub mod tasks;

#[cfg(test)]
pub mod test_utils;

pub type TxBatch = Vec<ProvenTransaction>;

/// TODO: Where to define this type?
pub struct Block {
    pub header: BlockHeader,
    pub body: BlockData
}

pub struct BlockData {
     pub updated_account_state_hashes: Vec<(AccountId, Digest)>,

     // TODO: Add remaining fields
}

