use std::sync::Arc;

use async_trait::async_trait;

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
