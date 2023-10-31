use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use miden_objects::{
    accounts::AccountId,
    crypto::merkle::{MmrPeaks, PartialMerkleTree},
    BlockHeader, Digest,
};

use crate::{block::Block, SharedProvenTx};

#[derive(Debug, PartialEq)]
pub enum TxInputsError {
    Dummy,
}

#[derive(Debug, PartialEq)]
pub enum BlockInputsError {
    Dummy,
}

#[derive(Debug, PartialEq)]
pub enum ApplyBlockError {
    Dummy,
}

#[async_trait]
pub trait ApplyBlock: Send + Sync + 'static {
    async fn apply_block(
        &self,
        block: Arc<Block>,
    ) -> Result<(), ApplyBlockError>;
}

/// Information needed from the store to verify a transaction
pub struct TxInputs {
    /// The account hash in the store corresponding to tx's account ID
    pub account_hash: Option<Digest>,

    /// Maps each consumed notes' nullifier to whether the note is already consumed
    pub nullifiers: BTreeMap<Digest, bool>,
}

/// Information needed from the store to build a block
pub struct BlockInputs {
    /// Previous block header
    pub block_header: BlockHeader,

    /// MMR peaks for the current chain state
    pub chain_peaks: MmrPeaks,

    /// latest account state hashes for all requested account IDs
    pub account_states: Vec<(AccountId, Digest)>,

    /// a partial Merkle Tree with paths to all requested accounts in the account TSMT
    pub account_proofs: PartialMerkleTree,

    /// a partial Merkle Tree with paths to all requested nullifiers in the account TSMT
    pub nullifier_proofs: PartialMerkleTree,
}

#[async_trait]
pub trait Store: ApplyBlock {
    async fn get_tx_inputs(
        &self,
        proven_tx: SharedProvenTx,
    ) -> Result<TxInputs, TxInputsError>;

    async fn get_block_inputs(
        &self,
        // updated_accounts: &[AccountId],
        updated_accounts: impl Iterator<Item = &AccountId> + Send,
        produced_nullifiers: impl Iterator<Item = &Digest> + Send,
    ) -> Result<BlockInputs, BlockInputsError>;
}
