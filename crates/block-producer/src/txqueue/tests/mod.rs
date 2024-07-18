use tokio::sync::mpsc::{self, error::TryRecvError};

use super::*;
use crate::{errors::BuildBatchError, test_utils::MockProvenTxBuilder, TransactionBatch};

// STRUCTS
// ================================================================================================

/// All transactions verify successfully
struct TransactionValidatorSuccess;

#[async_trait]
impl TransactionValidator for TransactionValidatorSuccess {
    async fn verify_tx(&self, _tx: &ProvenTransaction) -> Result<(), VerifyTxError> {
        Ok(())
    }
}

/// All transactions fail to verify
struct TransactionValidatorFailure;

#[async_trait]
impl TransactionValidator for TransactionValidatorFailure {
    async fn verify_tx(&self, tx: &ProvenTransaction) -> Result<(), VerifyTxError> {
        Err(VerifyTxError::AccountAlreadyModifiedByOtherTx(tx.account_id()))
    }
}

/// Records all batches built in `ready_batches`
struct BatchBuilderSuccess {
    ready_batches: mpsc::UnboundedSender<TransactionBatch>,
}

impl BatchBuilderSuccess {
    fn new(ready_batches: mpsc::UnboundedSender<TransactionBatch>) -> Self {
        Self { ready_batches }
    }
}

#[async_trait]
impl BatchBuilder for BatchBuilderSuccess {
    async fn build_batch(&self, txs: Vec<ProvenTransaction>) -> Result<(), BuildBatchError> {
        let batch = TransactionBatch::new(txs, Default::default())
            .expect("Tx batch building should have succeeded");
        self.ready_batches
            .send(batch)
            .expect("Sending to channel should have succeeded");

        Ok(())
    }
}

/// Always fails to build batch
#[derive(Default)]
struct BatchBuilderFailure;

#[async_trait]
impl BatchBuilder for BatchBuilderFailure {
    async fn build_batch(&self, txs: Vec<ProvenTransaction>) -> Result<(), BuildBatchError> {
        Err(BuildBatchError::TooManyNotesCreated(0, txs))
    }
}

// TESTS
// ================================================================================================

/// Tests that when the internal "build batch timer" hits, all transactions in the queue are sent to
/// be built in some batch
#[tokio::test(start_paused = true)]
#[miden_node_test_macro::enable_logging]
async fn test_build_batch_success() {
    let build_batch_frequency = Duration::from_millis(5);
    let batch_size = 3;
    let (sender, mut receiver) = mpsc::unbounded_channel::<TransactionBatch>();

    let tx_queue = Arc::new(TransactionQueue::new(
        Arc::new(TransactionValidatorSuccess),
        Arc::new(BatchBuilderSuccess::new(sender)),
        TransactionQueueOptions { build_batch_frequency, batch_size },
    ));

    // Starts the transaction queue task.
    tokio::spawn(tx_queue.clone().run());

    // the queue start empty
    assert_eq!(Err(TryRecvError::Empty), receiver.try_recv());

    // if no transactions have been added to the queue in the batch build interval, the queue does
    // nothing
    tokio::time::advance(build_batch_frequency).await;
    assert_eq!(Err(TryRecvError::Empty), receiver.try_recv(), "queue starts empty");

    // if there is a single transaction in the queue when it is time to build a batch, the batch is
    // created with that single transaction
    let tx = MockProvenTxBuilder::with_account_index(0).build();
    tx_queue
        .add_transaction(tx.clone())
        .await
        .expect("Transaction queue is running");

    tokio::time::advance(build_batch_frequency).await;
    let batch = receiver.try_recv().expect("Queue not empty");
    assert_eq!(
        Err(TryRecvError::Empty),
        receiver.try_recv(),
        "A single transaction produces a single batch"
    );
    let expected =
        TransactionBatch::new(vec![tx.clone()], Default::default()).expect("Valid transactions");
    assert_eq!(expected, batch, "The batch should have the one transaction added to the queue");

    // a batch will include up to `batch_size` transactions
    let mut txs = Vec::new();
    for _ in 0..batch_size {
        tx_queue
            .add_transaction(tx.clone())
            .await
            .expect("Transaction queue is running");
        txs.push(tx.clone())
    }
    tokio::time::advance(build_batch_frequency).await;
    let batch = receiver.try_recv().expect("Queue not empty");
    assert_eq!(
        Err(TryRecvError::Empty),
        receiver.try_recv(),
        "{batch_size} transactions create a single batch"
    );
    let expected = TransactionBatch::new(txs, Default::default()).expect("Valid transactions");
    assert_eq!(expected, batch, "The batch should the transactions to fill a batch");

    // the transaction queue eagerly produces batches
    let mut txs = Vec::new();
    for _ in 0..(2 * batch_size + 1) {
        tx_queue
            .add_transaction(tx.clone())
            .await
            .expect("Transaction queue is running");
        txs.push(tx.clone())
    }
    for expected_batch in txs.chunks(batch_size).map(|txs| txs.to_vec()) {
        tokio::time::advance(build_batch_frequency).await;
        let batch = receiver.try_recv().expect("Queue not empty");
        let expected =
            TransactionBatch::new(expected_batch, Default::default()).expect("Valid transactions");
        assert_eq!(expected, batch, "The batch should the transactions to fill a batch");
    }

    // ensure all transactions have been consumed
    tokio::time::advance(build_batch_frequency * 2).await;
    assert_eq!(
        Err(TryRecvError::Empty),
        receiver.try_recv(),
        "If there are no transactions, no batches are produced"
    );
}

/// Tests that when transactions fail to verify, they are not added to the queue
#[tokio::test(start_paused = true)]
#[miden_node_test_macro::enable_logging]
async fn test_tx_verify_failure() {
    let build_batch_frequency = Duration::from_millis(5);
    let batch_size = 3;

    let (sender, mut receiver) = mpsc::unbounded_channel::<TransactionBatch>();
    let batch_builder = Arc::new(BatchBuilderSuccess::new(sender));

    let tx_queue = Arc::new(TransactionQueue::new(
        Arc::new(TransactionValidatorFailure),
        batch_builder.clone(),
        TransactionQueueOptions { build_batch_frequency, batch_size },
    ));

    // Start the queue
    tokio::spawn(tx_queue.clone().run());

    // Add a bunch of transactions that will all fail tx verification
    for i in 0..(3 * batch_size as u32) {
        let r = tx_queue
            .add_transaction(MockProvenTxBuilder::with_account_index(i).build())
            .await;

        assert!(matches!(r, Err(AddTransactionError::VerificationFailed(_))));
        assert_eq!(
            Err(TryRecvError::Empty),
            receiver.try_recv(),
            "If there are no transactions, no batches are produced"
        );
    }
}

/// Tests that when batch building fails, transactions are added back to the ready queue
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_build_batch_failure() {
    let build_batch_frequency = Duration::from_millis(30);
    let batch_size = 3;

    let batch_builder = Arc::new(BatchBuilderFailure);

    let tx_queue = TransactionQueue::new(
        Arc::new(TransactionValidatorSuccess),
        batch_builder.clone(),
        TransactionQueueOptions { build_batch_frequency, batch_size },
    );

    let internal_ready_queue = tx_queue.ready_queue.clone();

    // Add enough transactions so that we have 1 batch
    for i in 0..batch_size {
        tx_queue
            .add_transaction(MockProvenTxBuilder::with_account_index(i as u32).build())
            .await
            .unwrap();
    }

    // Start the queue
    tokio::spawn(Arc::new(tx_queue).run());

    // Wait for tx queue to fail once to build the batch
    time::sleep(Duration::from_millis(45)).await;

    assert_eq!(internal_ready_queue.read().await.len(), 3);
}
