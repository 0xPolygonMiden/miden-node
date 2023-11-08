use miden_crypto::merkle::{MerklePath, MmrPeaks};
use miden_objects::{Digest, BlockHeader, accounts::AccountId};

pub struct AccountInputRecord {
    pub account_id: AccountId,
    pub account_hash: Digest,
    pub proof: MerklePath,
}

pub struct NullifierInputRecord {
    pub nullifier: Digest,
    pub proof: MerklePath,
}

/// Information needed from the store to build a block
pub struct BlockInputs {
    /// Previous block header
    pub block_header: BlockHeader,

    /// MMR peaks for the current chain state
    pub chain_peaks: MmrPeaks,

    /// The hashes of the requested accounts and their authentication paths
    pub account_states: Vec<AccountInputRecord>,

    /// The requested nullifiers and their authentication paths
    pub nullifiers: Vec<NullifierInputRecord>,
}
