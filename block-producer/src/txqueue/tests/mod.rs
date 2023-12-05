use tokio::time;

use super::*;
use crate::{
    batch_builder::{errors::BuildBatchError, TransactionBatch},
    test_utils::DummyProvenTxGenerator,
    SharedTxBatch,
};

// STRUCTS
// ================================================================================================

/// All transactions verify successfully
struct TransactionVerifierSuccess;

#[async_trait]
impl TransactionVerifier for TransactionVerifierSuccess {
    async fn verify_tx(
        &self,
        _tx: SharedProvenTx,
    ) -> Result<(), VerifyTxError> {
        Ok(())
    }
}

/// All transactions fail to verify
struct TransactionVerifierFailure;

#[async_trait]
impl TransactionVerifier for TransactionVerifierFailure {
    async fn verify_tx(
        &self,
        tx: SharedProvenTx,
    ) -> Result<(), VerifyTxError> {
        Err(VerifyTxError::AccountAlreadyModifiedByOtherTx(tx.account_id()))
    }
}

/// Records all batches built in `ready_batches`
#[derive(Default)]
struct BatchBuilderSuccess {
    ready_batches: SharedRwVec<SharedTxBatch>,
}

#[async_trait]
impl BatchBuilder for BatchBuilderSuccess {
    async fn build_batch(
        &self,
        txs: Vec<SharedProvenTx>,
    ) -> Result<(), BuildBatchError> {
        let batch = Arc::new(TransactionBatch::new(txs).unwrap());
        self.ready_batches.write().await.push(batch);

        Ok(())
    }
}

/// Always fails to build batch
#[derive(Default)]
struct BatchBuilderFailure;

#[async_trait]
impl BatchBuilder for BatchBuilderFailure {
    async fn build_batch(
        &self,
        _txs: Vec<SharedProvenTx>,
    ) -> Result<(), BuildBatchError> {
        Err(BuildBatchError::Dummy)
    }
}

// TESTS
// ================================================================================================

/// Tests that when the internal "build batch timer" hits, all transactions in the queue are sent to
/// be built in some batch
#[tokio::test]
async fn test_build_batch_success() {
    let build_batch_frequency = Duration::from_millis(5);
    let batch_size = 3;

    let batch_builder = Arc::new(BatchBuilderSuccess::default());

    let tx_queue = DefaultTransactionQueue::new(
        Arc::new(TransactionVerifierSuccess),
        batch_builder.clone(),
        DefaultTransactionQueueOptions {
            build_batch_frequency,
            batch_size,
        },
    );

    let proven_tx_generator = DummyProvenTxGenerator::new();

    // Add enough transactions so that we have 3 batches
    for _i in 0..(2 * batch_size + 1) {
        tx_queue
            .add_transaction(Arc::new(proven_tx_generator.dummy_proven_tx()))
            .await
            .unwrap();
    }

    // Start the queue
    tokio::spawn(tx_queue.run());

    // Wait for tx queue to build batches
    time::sleep(build_batch_frequency * 2).await;

    assert_eq!(batch_builder.ready_batches.read().await.len(), 3);
}

/// Tests that when transactions fail to verify, they are not added to the queue
#[tokio::test]
async fn test_tx_verify_failure() {
    let build_batch_frequency = Duration::from_millis(5);
    let batch_size = 3;

    let batch_builder = Arc::new(BatchBuilderSuccess::default());

    let tx_queue = DefaultTransactionQueue::new(
        Arc::new(TransactionVerifierFailure),
        batch_builder.clone(),
        DefaultTransactionQueueOptions {
            build_batch_frequency,
            batch_size,
        },
    );

    let internal_ready_queue = tx_queue.ready_queue.clone();

    let proven_tx_generator = DummyProvenTxGenerator::new();

    // Add a bunch of transactions that will all fail tx verification
    for _i in 0..(3 * batch_size) {
        let r = tx_queue.add_transaction(Arc::new(proven_tx_generator.dummy_proven_tx())).await;

        assert!(matches!(r, Err(AddTransactionError::VerificationFailed(_))));
    }

    // Start the queue
    tokio::spawn(tx_queue.run());

    // Wait for tx queue to build batches
    time::sleep(build_batch_frequency * 2).await;

    assert!(internal_ready_queue.read().await.is_empty());
    assert_eq!(batch_builder.ready_batches.read().await.len(), 0);
}

/// Tests that when batch building fails, transactions are added back to the ready queue
#[tokio::test]
async fn test_build_batch_failure() {
    let build_batch_frequency = Duration::from_millis(30);
    let batch_size = 3;

    let batch_builder = Arc::new(BatchBuilderFailure);

    let tx_queue = DefaultTransactionQueue::new(
        Arc::new(TransactionVerifierSuccess),
        batch_builder.clone(),
        DefaultTransactionQueueOptions {
            build_batch_frequency,
            batch_size,
        },
    );

    let internal_ready_queue = tx_queue.ready_queue.clone();

    let proven_tx_generator = DummyProvenTxGenerator::new();

    // Add enough transactions so that we have 1 batch
    for _i in 0..batch_size {
        tx_queue
            .add_transaction(Arc::new(proven_tx_generator.dummy_proven_tx()))
            .await
            .unwrap();
    }

    // Start the queue
    tokio::spawn(tx_queue.run());

    // Wait for tx queue to fail once to build the batch
    time::sleep(Duration::from_millis(45)).await;

    assert_eq!(internal_ready_queue.read().await.len(), 3);
}
