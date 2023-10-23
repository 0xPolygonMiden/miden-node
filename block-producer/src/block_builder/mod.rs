use std::{fmt::Debug, sync::Arc};

use async_trait::async_trait;

use crate::batch_builder::TransactionBatch;

#[async_trait]
pub trait BlockBuilder {
    type AddBatchesError: Debug;

    fn add_batches(batches: Vec<Arc<TransactionBatch>>) -> Result<(), Self::AddBatchesError>;
}
