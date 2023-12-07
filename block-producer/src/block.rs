use miden_objects::{accounts::AccountId, notes::NoteEnvelope, BlockHeader, Digest};

#[derive(Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub updated_accounts: Vec<(AccountId, Digest)>,
    pub created_notes: Vec<NoteEnvelope>,
    pub produced_nullifiers: Vec<Digest>,
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
