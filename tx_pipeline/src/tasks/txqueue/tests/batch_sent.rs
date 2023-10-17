use super::*;

/// Tests that when a batch is full (before the timer), it is sent.
///
/// We set a batch size of 3, and send 3 transactions with 10ms interval delay.
/// Transaction timer is set to infinite. After 50ms delay, we confirm that the batch was sent
#[tokio::test]
async fn test_batch_full_sent() {
    let interval_duration: Duration = Duration::from_millis(10);
    let batch_size: usize = 3;

    let handle_in = {
        let num_txs_to_send = batch_size;

        HandleInFixedInterval::new(interval_duration, num_txs_to_send)
    };
    let handle_out = HandleOutDefault::new();

    let tx_queue = TxQueue::new(
        handle_in,
        handle_out,
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::MAX,
        },
    );

    // Spawn tx_queue task
    {
        let tx_queue = tx_queue.clone();
        tokio::spawn(tx_queue.run());
    }

    time::sleep(Duration::from_millis(50)).await;

    // Ensure that the batch was sent
    assert_eq!(tx_queue.handle_out.batches.read().await.len(), 1);
    // Ensure that the batch contains all the transactions
    assert_eq!(tx_queue.handle_out.batches.read().await[0].len(), batch_size);
    // Ensure that the queue is empty
    assert!(tx_queue.ready_queue.lock().await.is_empty());
}

/// Tests that when a transaction's verification fails, it is not added to the queue.
///
/// We set a batch size of 3, and send 3 transactions with 10ms interval delay.
/// Transaction timer is set to 10ms. After 50ms delay, we confirm that no batch was sent.
#[tokio::test]
async fn test_tx_verification_failure() {
    let interval_duration: Duration = Duration::from_millis(10);
    let batch_size: usize = 3;

    let handle_in = {
        let num_txs_to_send = batch_size;

        HandleInFixedInterval::new(interval_duration, num_txs_to_send)
    };
    let handle_out = HandleOutFailVerification::new();

    let tx_queue = TxQueue::new(
        handle_in,
        handle_out,
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::MAX,
        },
    );

    // Spawn tx_queue task
    {
        let tx_queue = tx_queue.clone();
        tokio::spawn(tx_queue.run());
    }

    time::sleep(Duration::from_millis(50)).await;

    // Ensure that no batch was sent
    assert!(tx_queue.handle_out.batches.read().await.is_empty());
    // Ensure that the queue is empty
    assert!(tx_queue.ready_queue.lock().await.is_empty());
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
    let batch_size: usize = 3;

    let handle_in =
        HandleInVariableInterval::new(vec![Duration::from_millis(0), Duration::from_millis(20)]);
    let handle_out = HandleOutDefault::new();

    let tx_queue = TxQueue::new(
        handle_in,
        handle_out,
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::from_millis(30),
        },
    );

    // Spawn tx_queue task
    {
        let tx_queue = tx_queue.clone();
        tokio::spawn(tx_queue.run());
    }

    time::sleep(Duration::from_millis(40)).await;

    // Ensure that the batch was sent
    assert_eq!(tx_queue.handle_out.batches.read().await.len(), 1);
    // Ensure that the batch contains all the transactions
    assert_eq!(tx_queue.handle_out.batches.read().await[0].len(), batch_size - 1);
    // Ensure that the queue is empty
    assert!(tx_queue.ready_queue.lock().await.is_empty());
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

    let handle_in = HandleInVariableInterval::new(vec![
        Duration::from_millis(0),
        Duration::from_millis(10),
        Duration::from_millis(20),
    ]);
    let handle_out = HandleOutDefault::new();

    let tx_queue = TxQueue::new(
        handle_in,
        handle_out,
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::from_millis(40),
        },
    );

    // Spawn tx_queue task
    {
        let tx_queue = tx_queue.clone();
        tokio::spawn(tx_queue.run());
    }

    time::sleep(Duration::from_millis(55)).await;

    // Ensure that 1 batch was sent
    assert_eq!(tx_queue.handle_out.batches.read().await.len(), 1);
    // Ensure that the queue holds 1 transaction
    assert_eq!(tx_queue.ready_queue.lock().await.len(), 1);

    time::sleep(Duration::from_millis(35)).await;

    // Ensure that 2 batches were sent
    assert_eq!(tx_queue.handle_out.batches.read().await.len(), 2);
    // Ensure that first batch received is length 2
    assert_eq!(tx_queue.handle_out.batches.read().await[0].len(), 2);
    // Ensure that second batch received is length 1
    assert_eq!(tx_queue.handle_out.batches.read().await[1].len(), 1);
    // Ensure that the queue is empty
    assert!(tx_queue.ready_queue.lock().await.is_empty());
}
