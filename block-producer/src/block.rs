use miden_objects::{accounts::AccountId, BlockHeader, Digest};

// FIXME: Put this in miden-base?
pub struct Block {
    pub header: BlockHeader,
    pub state_commitments: StateCommitments,
    pub state_updates: StateUpdates,
    // TODO:
    // - full states for updated public accounts
    // - full states for created public notes
    // - zk proof
}

pub struct StateCommitments {
    pub account_db: Digest,
    pub note_db: Digest,
    pub nullifier_db: Digest,
}

pub struct StateUpdates {
    pub updated_account_state_hashes: Vec<(AccountId, Digest)>,
    pub consumed_notes_script_roots: Vec<Digest>,
    pub created_note_hashes: Vec<Digest>,
    pub new_nullifiers: Vec<Digest>,
}
