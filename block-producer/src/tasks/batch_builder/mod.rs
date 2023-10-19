#[cfg(test)]
mod tests;

use std::sync::Arc;

use async_trait::async_trait;

use crate::{msg::MessageHandler, SharedMutVec, SharedProvenTx};

type ReadyBatchQueue = SharedMutVec<TxBatch>;

/// A batch of transactions that have been proven with a single recursive proof.
///
/// FIXME: Properly define this type. For now, we store the proven transactions that go in the batch
pub struct TxBatch {
    proven_txs: Vec<SharedProvenTx>,
}

impl TxBatch {
    /// Returns the number of transactions in the batch
    pub fn num_txs(&self) -> usize {
        self.proven_txs.len()
    }
}


// Batch Builder task
// ================================================================================================
pub struct BatchBuilderTask {
}

// Batch Builder
// ================================================================================================

struct BatchBuilder {
    ready_batches: ReadyBatchQueue,
}

// Message handlers
// -------------------------------------------------------------------------------------------------

#[async_trait]
impl MessageHandler<Vec<SharedProvenTx>, ()> for BatchBuilder {
    /// Handler for transaction queue's `send_txs()` message
    async fn handle_message(
        self: Arc<Self>,
        proven_txs: Vec<SharedProvenTx>,
    ) {
        // Note: Normally, we would actually process the message to create the `TxBatch`.
        // We need to properly define the `TxBatch` type first
        let batch = TxBatch { proven_txs };

        self.ready_batches.lock().await.push(batch);
    }
}

#[async_trait]
impl MessageHandler<usize, Vec<TxBatch>> for BatchBuilder {
    /// Handler for block producer's `get_batches(max_num_txs)` message.
    ///
    /// `max_num_txs` is the maximum number of transactions that must be contained in the sum of all
    /// batches.
    async fn handle_message(
        self: Arc<Self>,
        max_num_txs: usize,
    ) -> Vec<TxBatch> {
        let mut locked_ready_batches = self.ready_batches.lock().await;

        let mut current_tx_count: usize = 0;
        let mut num_batches_to_send = 0;

        for batch in locked_ready_batches.iter() {
            if current_tx_count + batch.num_txs() < max_num_txs {
                num_batches_to_send += 1;
            } else {
                break;
            }

            current_tx_count += batch.num_txs();
        }

        locked_ready_batches.drain(..num_batches_to_send).collect()
    }
}
