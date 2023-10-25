use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use async_trait::async_trait;
use miden_objects::{accounts::AccountId, Digest};
use tokio::sync::RwLock;

use crate::{
    block::Block,
    store::GetTxInputs,
    txqueue::{TransactionVerifier, VerifyTxError},
    SharedProvenTx,
};

#[derive(Debug)]
pub enum ApplyBlockError {}

#[async_trait]
pub trait ApplyBlock {
    async fn apply_block(
        &self,
        block: Block,
    ) -> Result<(), ApplyBlockError>;
}

pub struct DefaulStateView<TI>
where
    TI: GetTxInputs,
{
    get_tx_inputs: Arc<TI>,

    /// The account ID of accounts being modified by transactions currently in the block production
    /// pipeline. We currently ensure that only 1 tx/block modifies any given account.
    accounts_in_flight: Arc<RwLock<BTreeSet<AccountId>>>,

    /// The nullifiers of notes consumed by transactions currently in the block production pipeline.
    nullifiers_in_flight: Arc<RwLock<BTreeSet<Digest>>>,
}

impl<TI> DefaulStateView<TI>
where
    TI: GetTxInputs,
{
    pub fn new(get_tx_inputs: Arc<TI>) -> Self {
        Self {
            get_tx_inputs,
            accounts_in_flight: Arc::new(RwLock::new(BTreeSet::new())),
            nullifiers_in_flight: Arc::new(RwLock::new(BTreeSet::new())),
        }
    }
}

#[async_trait]
impl<TI> TransactionVerifier for DefaulStateView<TI>
where
    TI: GetTxInputs,
{
    async fn verify_tx(
        &self,
        tx: SharedProvenTx,
    ) -> Result<(), VerifyTxError> {
        todo!()
    }
}

#[async_trait]
impl<TI> ApplyBlock for DefaulStateView<TI>
where
    TI: GetTxInputs,
{
    async fn apply_block(
        &self,
        block: Block,
    ) -> Result<(), ApplyBlockError> {
        todo!()
    }
}
