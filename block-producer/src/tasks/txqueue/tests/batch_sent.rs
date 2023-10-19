use super::*;

/// Tests that when a batch is full (before the timer), it is sent.
///
/// We set a batch size of 3, and send 3 transactions with 10ms interval delay.
/// Transaction timer is set to infinite. After 50ms delay, we confirm that the batch was sent
#[tokio::test]
async fn test_batch_full_sent() {
    let batch_size = 3;

    let (read_tx_client, ready_queue, batches) = setup(
        VerifyTxRpcSuccess,
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::MAX,
        },
    );

    // Start fixed interval client
    tokio::spawn(
        ReadTxClientFixedInterval::new(read_tx_client, Duration::from_millis(10), batch_size).run(),
    );

    time::sleep(Duration::from_millis(50)).await;

    // Ensure that the batch was sent
    assert_eq!(batches.lock().await.len(), 1);
    // Ensure that the batch contains all the transactions
    assert_eq!(batches.lock().await[0].len(), batch_size);
    // Ensure that the queue is empty
    assert!(ready_queue.lock().await.is_empty());
}

/// Tests that if `num_txs_in_queue > batch_size`, we still send a batch of
/// `batch_size`.
///
/// Note: in the future, this test *could* fail if when starting the queue, we
/// immediately check if a batch is ready.
#[tokio::test]
async fn test_proper_batch_size_sent() {
    let batch_size = 3;

    let (read_tx_client, ready_queue, batches) = setup(
        VerifyTxRpcSuccess,
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::MAX,
        },
    );

    // Fill queue so that `queue_size == batch_size`
    {
        let proven_tx_generator = DummyProvenTxGenerator::new();
        let mut locked_queue = ready_queue.lock().await;

        locked_queue.push(Arc::new(proven_tx_generator.dummy_proven_tx()));
        locked_queue.push(Arc::new(proven_tx_generator.dummy_proven_tx()));
        locked_queue.push(Arc::new(proven_tx_generator.dummy_proven_tx()));
    }

    // Start client that sends 1 transaction after 10ms
    tokio::spawn(
        ReadTxClientVariableInterval::new(read_tx_client, vec![Duration::from_millis(10)]).run(),
    );

    time::sleep(Duration::from_millis(50)).await;

    // Ensure that the batch was sent
    assert_eq!(batches.lock().await.len(), 1);
    // Ensure that the batch contains `batch_size` elements
    assert_eq!(batches.lock().await[0].len(), batch_size);
    // Ensure that the queue contains 1 transaction
    assert_eq!(ready_queue.lock().await.len(), 1);
}

/// Tests that when a transaction's verification fails, it is not added to the queue.
///
/// We set a batch size of 3, and send 3 transactions with 10ms interval delay.
/// Transaction timer is set to 10ms. After 50ms delay, we confirm that no batch was sent.
#[tokio::test]
async fn test_tx_verification_failure() {
    let batch_size = 3;

    let (read_tx_client, ready_queue, batches) = setup(
        VerifyTxRpcFailure,
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::MAX,
        },
    );

    // Start fixed interval client
    tokio::spawn(
        ReadTxClientFixedInterval::new(read_tx_client, Duration::from_millis(10), batch_size).run(),
    );

    time::sleep(Duration::from_millis(50)).await;

    // Ensure that no batch was sent
    assert!(batches.lock().await.is_empty());
    // Ensure that the queue is empty
    assert!(ready_queue.lock().await.is_empty());
}

/// Tests that if a batch is not full, then the batch will be sent regardless
/// due to the timer (which starts after the first transaction enters the
/// queue).
///
/// We set a batch size of 3, and send 2 transactions: after 0, and 20ms.
/// Transaction timer is set to 30ms. After 40ms delay, we confirm that the
/// batch was sent. This setup also ensures that the timer started after the
/// first transaction (as opposed to possibly the second one).
#[tokio::test]
async fn test_timer_send_batch() {
    let batch_size = 3;

    let (read_tx_client, ready_queue, batches) = setup(
        VerifyTxRpcSuccess,
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::from_millis(30),
        },
    );

    // Start client
    tokio::spawn(
        ReadTxClientVariableInterval::new(
            read_tx_client,
            vec![Duration::from_millis(0), Duration::from_millis(20)],
        )
        .run(),
    );

    time::sleep(Duration::from_millis(40)).await;

    // Ensure that the batch was sent
    assert_eq!(batches.lock().await.len(), 1);
    // Ensure that the batch contains all the transactions
    assert_eq!(batches.lock().await[0].len(), batch_size - 1);
    // Ensure that the queue is empty
    assert!(ready_queue.lock().await.is_empty());
}

/// This tests if the internal transaction timer properly resets after a full
/// batch is sent.
///
/// We set the batch size to 2, send 3 transactions (see diagram), and confirm
/// that 2 were properly sent initially. The tx timer is set to 40ms. After
/// 55ms, we confirm that we have only 1 batch sent (it would be 2 if the first
/// timer was not reset). After 80ms, ensure that both batches were sent.
///
///        timer                             confirm sent         confirm sent
///        reset                             1 batch               2 batches
///  tx1    tx2             tx3             
///  |       |       |       |       |       |       |       |       |
///  0ms    10ms    20ms    30ms    40ms   50ms    60ms    70ms     80ms
///  |-------timer (cancelled)-------|
///                          |--------------timer------------|
///
/// Specifically, we're ensuring that tx3 is not sent early (at 50ms), which
/// would happen if the initial timer was not properly cancelled
#[tokio::test]
async fn test_tx_timer_resets_after_full_batch_sent() {
    let batch_size: usize = 2;

    let (read_tx_client, ready_queue, batches) = setup(
        VerifyTxRpcSuccess,
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::from_millis(40),
        },
    );

    // Start client
    tokio::spawn(
        ReadTxClientVariableInterval::new(
            read_tx_client,
            vec![Duration::from_millis(0), Duration::from_millis(10), Duration::from_millis(20)],
        )
        .run(),
    );

    time::sleep(Duration::from_millis(55)).await;

    // Ensure that 1 batch was sent
    assert_eq!(batches.lock().await.len(), 1);
    // Ensure that the queue holds 1 transaction
    assert_eq!(ready_queue.lock().await.len(), 1);

    time::sleep(Duration::from_millis(35)).await;

    // Ensure that 2 batches were sent
    assert_eq!(batches.lock().await.len(), 2);
    // Ensure that first batch received is length 2
    assert_eq!(batches.lock().await[0].len(), 2);
    // Ensure that second batch received is length 1
    assert_eq!(batches.lock().await[1].len(), 1);
    // Ensure that the queue is empty
    assert!(ready_queue.lock().await.is_empty());
}

/// This tests if the internal transaction timer properly resets after a
/// timeout. Specifically, we test that we don't mistakenly start creating
/// batches at fixed interval after a first timeout occurs.
///
/// We set the batch size to 2, send 1 transaction and wait for timeout (30ms)
/// so that it is sent in batch. We then send another transaction, and confirm
/// that it is still in the queue after 60ms (it wouldn't be if we forgot to
/// reset the timeout).
///
///
///                                                           confirm tx2
///                                                           in queue
///  tx1                                tx2
///  |       |       |       |       |       |       |       |       |
///  0ms    10ms    20ms    30ms    40ms   50ms    60ms    70ms     80ms
///  |---------timer---------|
///                          |------timer (if bug)---|
///                                          |---------timer---------|
///
/// Specifically, we're ensuring that tx2 is still in the queue at 75ms, which
/// wouldn't be the case if we forgot to reset the timer after tx1 is sent in a
/// batch.
#[tokio::test]
async fn test_tx_timer_resets_after_timeout() {
    let batch_size: usize = 2;

    let (read_tx_client, ready_queue, batches) = setup(
        VerifyTxRpcSuccess,
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::from_millis(30),
        },
    );

    // Start client
    tokio::spawn(
        ReadTxClientVariableInterval::new(
            read_tx_client,
            vec![Duration::from_millis(0), Duration::from_millis(45)],
        )
        .run(),
    );

    time::sleep(Duration::from_millis(75)).await;

    // Ensure that 1 batch was sent
    assert_eq!(batches.lock().await.len(), 1);
    // Ensure that the queue holds 1 transaction
    assert_eq!(ready_queue.lock().await.len(), 1);

    time::sleep(Duration::from_millis(35)).await;

    // Ensure that 2 batches were sent
    assert_eq!(batches.lock().await.len(), 2);
    // Ensure that first batch received is length 1
    assert_eq!(batches.lock().await[0].len(), 1);
    // Ensure that second batch received is length 1
    assert_eq!(batches.lock().await[1].len(), 1);
    // Ensure that the queue is empty
    assert!(ready_queue.lock().await.is_empty());
}

// HELPERS
// ================================================================================================

/// Starts the RPC servers (txqueue's server and servers which txqueue is a client).
/// Returns handles useful for tests
fn setup<VerifyTxServerImpl>(
    verify_tx_server_impl: VerifyTxServerImpl,
    tx_queue_options: TxQueueOptions,
) -> (ReadTxRpcClient, ReadyQueue, SharedMutVec<Vec<SharedProvenTx>>)
where
    VerifyTxServerImpl: ServerImpl<SharedProvenTx, Result<(), VerifyTxError>>,
{
    let (verify_tx_client, verify_tx_server) = create_client_server_pair(verify_tx_server_impl);

    let send_txs_server_impl = SendTxsDefaultServerImpl::new();
    let batches = send_txs_server_impl.batches.clone();
    let (send_txs_client, send_txs_server) = create_client_server_pair(send_txs_server_impl);

    let tx_queue = TxQueue::new(verify_tx_client, send_txs_client, tx_queue_options);

    let ready_queue = tx_queue.ready_queue.clone();
    let (read_tx_client, read_tx_server) = create_client_server_pair(tx_queue);

    // Start servers
    tokio::spawn(verify_tx_server.serve());
    tokio::spawn(send_txs_server.serve());
    tokio::spawn(read_tx_server.serve());

    (read_tx_client, ready_queue, batches)
}
