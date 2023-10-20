use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use std::{fmt::Debug, sync::Arc};

#[async_trait]
pub trait StateViewTrait: Send + Sync + 'static {
    type VerifyTxError: Debug;

    async fn verify_tx(&self) -> Result<Arc<ProvenTransaction>, Self::VerifyTxError>;
}
