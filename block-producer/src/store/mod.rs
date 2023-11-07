use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use miden_objects::{
    accounts::AccountId,
    crypto::merkle::MmrPeaks,
    BlockHeader, Digest,
};
use miden_vm::crypto::MerklePath;

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
