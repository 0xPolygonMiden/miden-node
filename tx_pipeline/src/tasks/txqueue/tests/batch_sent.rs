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

        HandleInInterval::new(interval_duration, num_txs_to_send)
    };
    let handle_out = HandleOutDefault::new();

    tokio::spawn(tx_queue_task(
        handle_in,
        handle_out.clone(),
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: Duration::MAX,
        },
    ));

    time::sleep(batch_size as u32 * interval_duration + interval_duration).await;

    // Ensure that the batch was sent
    assert_eq!(handle_out.batches.read().await.len(), 1);
    // Ensure that the batch contains all the transactions
    assert_eq!(handle_out.batches.read().await[0].len(), batch_size);
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

        HandleInInterval::new(interval_duration, num_txs_to_send)
    };
    let handle_out = HandleOutFailVerification::new();

    tokio::spawn(tx_queue_task(
        handle_in,
        handle_out.clone(),
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: interval_duration,
        },
    ));

    time::sleep(batch_size as u32 * interval_duration + interval_duration).await;

    // Ensure that no batch was sent
    assert!(handle_out.batches.read().await.is_empty());
}

/// Tests that if a batch is not full, then the batch will be sent regardless
/// due to the timer (which starts after the first transaction enters the
/// queue).
///
/// We set a batch size of 3, and send 2 transactions with 10ms interval delay.
/// Transaction timer is set to 30ms. After 35ms delay, we confirm that the
/// batch was sent. Checking after 35ms delay also ensures that the timer
/// started after the first transaction (as opposed to possibly the second one).
#[tokio::test]
async fn test_timer_send_batch() {
    let interval_duration: Duration = Duration::from_millis(10);
    let batch_size: usize = 3;

    let handle_in = {
        let num_txs_to_send = batch_size - 1;

        HandleInInterval::new(interval_duration, num_txs_to_send)
    };
    let handle_out = HandleOutDefault::new();

    tokio::spawn(tx_queue_task(
        handle_in,
        handle_out.clone(),
        TxQueueOptions {
            batch_size,
            tx_max_time_in_queue: interval_duration * 3,
        },
    ));

    time::sleep(batch_size as u32 * interval_duration + (interval_duration / 2)).await;

    // Ensure that the batch was sent
    assert_eq!(handle_out.batches.read().await.len(), 1);
    // Ensure that the batch contains all the transactions
    assert_eq!(handle_out.batches.read().await[0].len(), batch_size - 1);
}
