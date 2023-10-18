use std::{cmp::min, sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::sync::{mpsc::UnboundedSender, Mutex};

use crate::{
    rpc::{Rpc, RpcClient},
    SharedProvenTx,
};

// TODO: Put in right module
pub enum VerifyTxError {}

#[derive(Debug)]
pub enum SendTxsError {}

// TYPE ALIASES
// ================================================================================================

type SharedMutVec<T> = Arc<Mutex<Vec<T>>>;
type ReadyQueue = SharedMutVec<SharedProvenTx>;

/// Configuration parameters for the transaction queue
#[derive(Clone, Debug)]
pub struct TxQueueOptions {
    /// The size of a batch. When the internal queue reaches this value, the queued transactions
    /// will be sent to be batched.
    pub batch_size: usize,
    /// The maximum time a transaction should sit in the transaction queue before being batched
    pub tx_max_time_in_queue: Duration,
}

// READ TX SERVER
// ================================================================================================

/// Server which receives transactions, verifies, and adds them to an internal queue
pub struct ReadTxRpc {
    verify_tx_client: RpcClient<SharedProvenTx, Result<(), VerifyTxError>>,
    send_txs_client: RpcClient<Vec<SharedProvenTx>, ()>,
    ready_queue: ReadyQueue,
    timer_task_handle: TimerTaskHandle,
    options: TxQueueOptions,
}

#[async_trait]
impl Rpc<ProvenTransaction, ()> for ReadTxRpc {
    async fn handle_request(
        self: Arc<Self>,
        proven_tx: ProvenTransaction,
    ) {
        let proven_tx = Arc::new(proven_tx);

        let verification_result =
            self.verify_tx_client.call(proven_tx.clone()).expect("verify_tx_client");

        if let Err(_failure_reason) = verification_result.await {
            // TODO: Log failure properly
            return;
        }

        // Transaction verification succeeded. It is safe to add transaction to queue.
        let mut locked_ready_queue = self.ready_queue.lock().await;

        if locked_ready_queue.is_empty() {
            self.timer_task_handle.start_timer();
        }

        locked_ready_queue.push(proven_tx);

        if locked_ready_queue.len() >= self.options.batch_size {
            // We are sending a batch, so reset the timer
            self.timer_task_handle.stop_timer();

            let send_txs_client = self.send_txs_client.clone();
            let txs_in_batch = drain_queue(&mut locked_ready_queue, self.options.batch_size);

            tokio::spawn(send_batch(txs_in_batch, send_txs_client));
        }
    }
}

// TIMER TASK
// ================================================================================================

/// Represents a channel of communication with the timer task.
#[derive(Clone)]
struct TimerTaskHandle {
    sender: UnboundedSender<TimerMessage>,
}

impl TimerTaskHandle {
    pub fn start_timer(&self) {
        self.sender
            .send(TimerMessage::StartTimer)
            .expect("failed to send on timer channel");
    }

    pub fn stop_timer(&self) {
        self.sender
            .send(TimerMessage::StopTimer)
            .expect("failed to send on timer channel");
    }
}

/// Encapsulates all messages that can be sent to the timer task
enum TimerMessage {
    StartTimer,
    StopTimer,
}

// HELPERS
// ================================================================================================

/// Drains at most `batch_size` from the queue
fn drain_queue(
    locked_ready_queue: &mut Vec<SharedProvenTx>,
    batch_size: usize,
) -> Vec<SharedProvenTx> {
    let num_to_drain = min(batch_size, locked_ready_queue.len());
    locked_ready_queue.drain(..num_to_drain).collect()
}

/// This task is responsible for ensuring that the batch is successfully sent, whether this requires
/// retries, or any other strategy.
async fn send_batch(
    txs_in_batch: Vec<SharedProvenTx>,
    send_txs_client: RpcClient<Vec<SharedProvenTx>, ()>,
) {
    if txs_in_batch.is_empty() {
        return;
    }

    // Panic for now if the send fails. In the future, we might want a more sophisticated strategy,
    // such as retrying, or something else.
    send_txs_client
        .call(txs_in_batch)
        .expect("send batch expected to succeed")
        .await
        .expect("send batch expected to succeed");
}
