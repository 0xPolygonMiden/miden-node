use std::collections::BTreeMap;

use miden_objects::{
    accounts::AccountId,
    notes::{NoteEnvelope, Nullifier},
    BlockHeader, Digest,
};

#[derive(Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub updated_accounts: Vec<(AccountId, Digest)>,
    pub created_notes: BTreeMap<u64, NoteEnvelope>,
    pub produced_nullifiers: Vec<Nullifier>,
    // TODO:
    // - full states for updated public accounts
    // - full states for created public notes
    // - zk proof
}

impl Block {
    pub fn hash(&self) -> Digest {
        todo!()
    }
}
