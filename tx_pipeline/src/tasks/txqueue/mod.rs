use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::{select, sync::Mutex};

#[async_trait]
pub trait TxQueueHandleIn: Send + Sync + 'static {
    type ReadTxError: Debug;

    async fn read_transaction(&self) -> Result<ProvenTransaction, Self::ReadTxError>;
}

#[async_trait]
pub trait TxQueueHandleOut: Send + Sync + 'static {
    type VerifyTxError: Debug;
    type TxVerificationFailureReason: Debug + Send;
    type ProduceBatchError: Debug;

    async fn verify_transaction(
        &self,
        tx: Arc<ProvenTransaction>,
    ) -> Result<Result<(), Self::TxVerificationFailureReason>, Self::VerifyTxError>;

    async fn produce_batch(
        &self,
        txs: Vec<ProvenTransaction>,
    ) -> Result<(), Self::ProduceBatchError>;
}

pub struct TxQueueOptions {
    pub batch_size: usize,
}

pub async fn tx_queue<HandleIn, HandleOut>(
    handle_in: HandleIn,
    handle_out: HandleOut,
    options: TxQueueOptions,
) where
    HandleIn: TxQueueHandleIn,
    HandleOut: TxQueueHandleOut,
{
    let queue_task = TxQueue::new(handle_in, handle_out, options);
    queue_task.run().await
}

struct TxQueue<HandleIn, HandleOut>
where
    HandleIn: TxQueueHandleIn,
    HandleOut: TxQueueHandleOut,
{
    ready_queue: Arc<Mutex<Vec<Arc<ProvenTransaction>>>>,
    handle_in: Arc<HandleIn>,
    handle_out: Arc<HandleOut>,
    options: TxQueueOptions,
}

impl<HandleIn, HandleOut> TxQueue<HandleIn, HandleOut>
where
    HandleIn: TxQueueHandleIn,
    HandleOut: TxQueueHandleOut,
{
    pub fn new(
        handle_in: HandleIn,
        handle_out: HandleOut,
        options: TxQueueOptions,
    ) -> Self {
        Self {
            ready_queue: Arc::new(Mutex::new(Vec::new())),
            handle_in: Arc::new(handle_in),
            handle_out: Arc::new(handle_out),
            options,
        }
    }

    pub async fn run(self) {
        let tx_queue = Arc::new(self);
        loop {
            select! {
                // Handle new transaction coming in
                proven_tx = tx_queue.handle_in.read_transaction() => {
                    let proven_tx = proven_tx.expect("Failed to read transaction");
                    let tx_queue = tx_queue.clone();
                    tokio::spawn(async move {
                        on_read_transaction(tx_queue, proven_tx).await
                    });
                }
            }
        }
    }
}

async fn on_read_transaction<HandleIn, HandleOut>(
    tx_queue: Arc<TxQueue<HandleIn, HandleOut>>,
    proven_tx: ProvenTransaction,
) where
    HandleIn: TxQueueHandleIn,
    HandleOut: TxQueueHandleOut,
{
    let proven_tx = Arc::new(proven_tx);

    let verification_passed = tx_queue
        .handle_out
        .verify_transaction(proven_tx.clone())
        .await
        .expect("Failed to verify transaction");

    if let Err(failure_reason) = verification_passed {
        // TODO: Log failure properly
        println!("Transaction verification failed with reason: {failure_reason:?}");
        return;
    }

    // Transaction verification succeeded. It is safe to add transaction to queue.
    let mut ready_queue = tx_queue.ready_queue.lock().await;
    ready_queue.push(proven_tx);
}
