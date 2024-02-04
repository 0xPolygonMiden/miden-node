use miden_crypto::merkle::{MerklePath, MmrPeaks};
use miden_objects::{accounts::AccountId, BlockHeader, Digest};

#[derive(Clone, Debug)]
pub struct AccountInputRecord {
    pub account_id: AccountId,
    pub account_hash: Digest,
    pub proof: MerklePath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NullifierInputRecord {
    pub nullifier: Digest,
    pub proof: MerklePath,
}

/// Information needed from the store to build a block
#[derive(Clone, Debug)]
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
