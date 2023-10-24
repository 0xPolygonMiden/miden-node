use async_trait::async_trait;

use crate::{
    block::Block,
    txqueue::{TransactionVerifier, VerifyTxError},
    SharedProvenTx,
};

#[derive(Debug)]
pub enum ApplyBlockError {}

#[async_trait]
pub trait ApplyBlock {
    async fn apply_block(&self, block: Block) -> Result<(), ApplyBlockError>;
}

pub struct DefaulStateView {}

#[async_trait]
impl TransactionVerifier for DefaulStateView {
    async fn verify_tx(
        &self,
        tx: SharedProvenTx,
    ) -> Result<(), VerifyTxError> {
        todo!()
    }
}

#[async_trait]
impl ApplyBlock for DefaulStateView {
    async fn apply_block(&self, block: Block) -> Result<(), ApplyBlockError> {
        todo!()
    }
}
