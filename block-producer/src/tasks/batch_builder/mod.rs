use std::sync::Arc;

use async_trait::async_trait;

use crate::{rpc::ServerImpl, SharedMutVec, SharedProvenTx};

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
impl ServerImpl<Vec<SharedProvenTx>, ()> for BatchBuilder {
    async fn handle_request(
        self: Arc<Self>,
        request: Vec<SharedProvenTx>,
    ) {
        todo!()
    }
}

/// Handle for block producer's `get_batches()` message
#[async_trait]
impl ServerImpl<(), Vec<TxBatch>> for BatchBuilder {
    async fn handle_request(
        self: Arc<Self>,
        _request: (),
    ) -> Vec<TxBatch> {
        todo!()
    }
}
