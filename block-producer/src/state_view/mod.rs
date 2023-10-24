use async_trait::async_trait;

use crate::{
    txqueue::{TransactionVerifier, VerifyTxError},
    SharedProvenTx,
};

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
