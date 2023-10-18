use std::{cmp::min, sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::{
    select,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
    time::{sleep, Sleep},
};

use crate::{
    rpc::{Rpc, RpcClient, RpcServer},
    SharedProvenTx,
};

// TODO: Put in right module
#[derive(Clone, Debug)]
pub enum VerifyTxError {}

#[derive(Clone, Debug)]
pub enum SendTxsError {}

// TYPE ALIASES
// ================================================================================================

type SharedMutVec<T> = Arc<Mutex<Vec<T>>>;
type ReadyQueue = SharedMutVec<SharedProvenTx>;
type ReadTxRpcServer = RpcServer<ProvenTransaction, (), ReadTxRpc>;

// TX QUEUE
// ================================================================================================

/// The transaction queue task
pub struct TxQueue {
    verify_tx_client: RpcClient<SharedProvenTx, Result<(), VerifyTxError>>,
    send_txs_client: RpcClient<Vec<SharedProvenTx>, ()>,
    ready_queue: ReadyQueue,
    timer_task_handle: TimerTaskHandle,
    options: TxQueueOptions,
}

impl TxQueue {
    pub fn new(
        verify_tx_client: RpcClient<SharedProvenTx, Result<(), VerifyTxError>>,
        send_txs_client: RpcClient<Vec<SharedProvenTx>, ()>,
        ready_queue: ReadyQueue,
        options: TxQueueOptions,
    ) -> Self {
        let timer_task_handle = start_timer_task(
            ready_queue.clone(),
            send_txs_client.clone(),
            options.tx_max_time_in_queue,
            options.batch_size,
        );

        Self {
            verify_tx_client,
            send_txs_client,
            ready_queue,
            timer_task_handle,
            options,
        }
    }

    pub fn get_read_tx_rpc(&self) -> ReadTxRpc {
        ReadTxRpc {
            verify_tx_client: self.verify_tx_client.clone(),
            send_txs_client: self.send_txs_client.clone(),
            ready_queue: self.ready_queue.clone(),
            timer_task_handle: self.timer_task_handle.clone(),
            options: self.options.clone(),
        }
    }

    // Start the task
    pub async fn run(self, read_tx_rpc_server: ReadTxRpcServer) {
        read_tx_rpc_server.serve().await.expect("read_tx_rpc_server closed")
    }
}

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

fn start_timer_task(
    ready_queue: ReadyQueue,
    send_txs_client: RpcClient<Vec<SharedProvenTx>, ()>,
    tx_max_time_in_queue: Duration,
    batch_size: usize,
) -> TimerTaskHandle {
    let (timer_task, handle) =
        TimerTask::new(ready_queue, send_txs_client, tx_max_time_in_queue, batch_size);

    tokio::spawn(timer_task.run());

    handle
}

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

/// Manages the transaction timer, which ensures that no transaction sits in the queue for longer
/// than [`TxQueueOptions::tx_max_time_in_queue`]. Is responsible for sending the batch when the
/// timer expires.
///
struct TimerTask {
    ready_queue: ReadyQueue,
    receiver: UnboundedReceiver<TimerMessage>,
    send_txs_client: RpcClient<Vec<SharedProvenTx>, ()>,
    tx_max_time_in_queue: Duration,
    batch_size: usize,
}

impl TimerTask {
    pub fn new(
        ready_queue: ReadyQueue,
        send_txs_client: RpcClient<Vec<SharedProvenTx>, ()>,
        tx_max_time_in_queue: Duration,
        batch_size: usize,
    ) -> (Self, TimerTaskHandle) {
        let (sender, receiver) = unbounded_channel();

        (
            Self {
                ready_queue,
                receiver,
                send_txs_client,
                tx_max_time_in_queue,
                batch_size,
            },
            TimerTaskHandle { sender },
        )
    }

    async fn run(mut self) {
        let mut sleep_duration = Duration::MAX;

        loop {
            let send_batch_timer: Sleep = sleep(sleep_duration);

            select! {
                maybe_msg = self.receiver.recv() => {
                    let msg = maybe_msg.expect("Failed to receive on timer channel");
                    match msg {
                        TimerMessage::StartTimer => sleep_duration = self.tx_max_time_in_queue,
                        TimerMessage::StopTimer => sleep_duration = Duration::MAX,
                    }
                }
                () = send_batch_timer => {
                    let mut locked_ready_queue = self.ready_queue.lock().await;
                    let txs_in_batch = drain_queue(&mut locked_ready_queue, self.batch_size);
                    tokio::spawn(send_batch(txs_in_batch, self.send_txs_client.clone()));
                    sleep_duration = Duration::MAX;
                }
            }
        }
    }
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
