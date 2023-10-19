use std::sync::Arc;

use async_trait::async_trait;

use crate::{msg::MessageHandler, SharedMutVec, SharedProvenTx};

/// A batch of transactions that have been proven with a single recursive proof.
///
/// FIXME: Properly define this type. For now, we store the proven transactions that go in the batch
pub struct TxBatch {
    proven_txs: Vec<SharedProvenTx>,
}

type ReadyBatchQueue = SharedMutVec<TxBatch>;

// Batch Builder
// ================================================================================================

struct BatchBuilder {
    ready_batches: ReadyBatchQueue,
}

// Message handlers
// -------------------------------------------------------------------------------------------------

/// Handler for transaction queue's `send_txs()` message
#[async_trait]
impl MessageHandler<Vec<SharedProvenTx>, ()> for BatchBuilder {
    async fn handle_message(
        self: Arc<Self>,
        message: Vec<SharedProvenTx>,
    ) {
        todo!()
    }
}

/// Handle for block producer's `get_batches()` message
#[async_trait]
impl MessageHandler<(), Vec<TxBatch>> for BatchBuilder {
    async fn handle_message(
        self: Arc<Self>,
        _message: (),
    ) -> Vec<TxBatch> {
        todo!()
    }
}
