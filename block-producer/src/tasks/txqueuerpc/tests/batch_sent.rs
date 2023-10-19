use super::*;

/// Tests that when a batch is full (before the timer), it is sent.
///
/// We set a batch size of 3, and send 3 transactions with 10ms interval delay.
/// Transaction timer is set to infinite. After 50ms delay, we confirm that the batch was sent
#[tokio::test]
async fn test_batch_full_sent() {
    let batch_size = 3;

    let (verify_tx_client, verify_tx_server) = create_client_server_pair(VerifyTxRpcSuccess);

    let send_txs_server_impl = SendTxsDefaultServerImpl::new();
    let batches = send_txs_server_impl.batches.clone();
    let (send_txs_client, send_txs_server) = create_client_server_pair(send_txs_server_impl);

    let tx_queue = TxQueue::new(
        verify_tx_client,
        send_txs_client,
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::MAX,
        },
    );
    let ready_queue = tx_queue.ready_queue.clone();
    let (read_tx_client, read_tx_server) = create_client_server_pair(tx_queue);

    // Start servers
    tokio::spawn(verify_tx_server.serve());
    tokio::spawn(send_txs_server.serve());
    tokio::spawn(read_tx_server.serve());

    // Start fixed interval client
    tokio::spawn(
        ReadTxClientFixedInterval::new(read_tx_client, Duration::from_millis(10), 3).run(),
    );

    time::sleep(Duration::from_millis(50)).await;

    // Ensure that the batch was sent
    assert_eq!(batches.lock().await.len(), 1);
    // Ensure that the batch contains all the transactions
    assert_eq!(batches.lock().await[0].len(), batch_size);
    // Ensure that the queue is empty
    assert!(ready_queue.lock().await.is_empty());
}
