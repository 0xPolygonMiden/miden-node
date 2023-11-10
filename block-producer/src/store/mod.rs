use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use miden_node_proto::domain::BlockInputs;
use miden_objects::{accounts::AccountId, Digest};
use thiserror::Error;

use crate::{block::Block, SharedProvenTx};

#[derive(Debug, PartialEq)]
pub enum TxInputsError {
    Dummy,
}

#[derive(Debug, PartialEq, Eq, Error)]
pub enum BlockInputsError {
    #[error("dummy")]
    Dummy,
}

#[derive(Debug, PartialEq, Eq, Error)]
pub enum ApplyBlockError {
    #[error("dummy")]
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
