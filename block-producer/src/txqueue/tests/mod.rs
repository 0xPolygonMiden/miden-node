use super::*;
use crate::batch_builder::{BuildBatchError, TransactionBatch};

/// All transactions verify successfully
struct TransactionVerifierSuccess;

#[async_trait]
impl TransactionVerifier for TransactionVerifierSuccess {
    async fn verify_tx(
        &self,
        _tx: SharedProvenTx,
    ) -> Result<(), VerifyTxError> {
        Ok(())
    }
}

/// Records all batches built in `ready_batches`
struct MockBatchBuilder {
    ready_batches: SharedRwVec<Arc<TransactionBatch>>,
}

#[async_trait]
impl BatchBuilder for MockBatchBuilder {
    async fn build_batch(
        &self,
        txs: Vec<SharedProvenTx>,
    ) -> Result<(), BuildBatchError> {
        let batch = Arc::new(TransactionBatch::new(txs));
        self.ready_batches.write().await.push(batch);

        Ok(())
    }
}
