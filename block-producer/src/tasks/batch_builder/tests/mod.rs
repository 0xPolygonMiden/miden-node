use tokio::task::JoinSet;

// Tests to have
// + get_batches: test that *at most* num_txs are sent
use super::*;

use crate::test_utils::DummyProvenTxGenerator;

/// Tests that batches sent are added to the internal batch queue
/// 
/// FIXME: This test will properly fail after we implement a proper `TxBatch`. We can fix it by
/// sending proper proven transactions.
#[tokio::test]
async fn test_batch_added_to_queue() {
    let num_batches = 3;

    let (batch_builder, send_txs_sender, _get_batches_sender) = setup();

    let proven_tx_generator = DummyProvenTxGenerator::new();
    let batch = vec![Arc::new(proven_tx_generator.dummy_proven_tx())];

    let mut set = JoinSet::new();

    for _i in 0..num_batches {
        set.spawn(send_txs_sender.call(batch.clone()).unwrap());
    }

    // Wait for all batches to be done sending
    while let Some(_) = set.join_next().await {}

    // Ensure that all batches sent were added to the queue
    assert_eq!(batch_builder.ready_batches.lock().await.len(), num_batches);
}

// HELPERS
// ================================================================================================

fn setup() -> (Arc<BatchBuilder>, SendTxsMessageSender, GetBatchesMessageSender) {
    let batch_builder = Arc::new(BatchBuilder::new());
    let (send_txs_sender, send_txs_recv) =
        create_message_sender_receiver_pair(batch_builder.clone());
    let (get_batches_sender, get_batches_recv) =
        create_message_sender_receiver_pair(batch_builder.clone());

    tokio::spawn(async move {
        send_txs_recv.serve().await.expect("send_txs_recv message receiver failed");
    });

    tokio::spawn(async move {
        get_batches_recv
            .serve()
            .await
            .expect("get_batches_recv message receiver failed")
    });

    (batch_builder, send_txs_sender, get_batches_sender)
}
