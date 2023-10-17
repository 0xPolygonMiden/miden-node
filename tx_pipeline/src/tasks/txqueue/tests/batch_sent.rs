use super::*;

/// Tests that when a batch is full (before the timer), it is sent.
///
/// We set a batch size of 3, and send 3 transactions with 10ms interval delay.
/// Transaction timer is set to infinite. After 40ms delay, we confirm that the batch was sent
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

    time::sleep(batch_size as u32 * interval_duration + interval_duration).await;

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
/// Transaction timer is set to 10ms. After 40ms delay, we confirm that no batch was sent.
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

    time::sleep(batch_size as u32 * interval_duration + interval_duration).await;

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
