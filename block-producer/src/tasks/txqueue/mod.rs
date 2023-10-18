//! The transaction queue takes transactions coming in, validates them, and eventually sends them
//! out in a batch. We say "sending a batch" to represent handing over a set of transactions to the
//! batch builder.
//!
//! Specifically, the requirements are:
//! - A transaction that fails validation is dropped
//! - There are 2 conditions for a batch to be sent:
//!   1. The internal queue size reaches [`TxQueueOptions::batch_size`]
//!   2. A transaction in the internal queue has been sitting for more than
//!      [`TxQueueOptions::tx_max_time_in_queue`]

#[cfg(test)]
mod tests;

use std::cmp::min;
use std::sync::Arc;
use std::{fmt::Debug, time::Duration};

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::select;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio::time::{sleep, Sleep};

// TYPE ALIASES
// ================================================================================================

type SharedProvenTx = Arc<ProvenTransaction>;
type SharedVec<T> = Arc<Mutex<Vec<T>>>;
type ReadyQueue = SharedVec<SharedProvenTx>;

// PUBLIC INTERFACE
// ================================================================================================

/// Contains all the methods for the transaction queue to fetch incoming data.
#[async_trait]
pub trait TxQueueHandleIn: Send + Sync + 'static {
    type ReadTxError: Debug;

    async fn read_transaction(&self) -> Result<ProvenTransaction, Self::ReadTxError>;
}

/// Contains all the methods for the transaction queue to send messages out.
#[async_trait]
pub trait TxQueueHandleOut: Send + Sync + 'static {
    type VerifyTxError: Debug;
    type TxVerificationFailureReason: Debug + Send;
    type ProduceBatchError: Debug;

    async fn verify_transaction(
        &self,
        tx: Arc<ProvenTransaction>,
    ) -> Result<Result<(), Self::TxVerificationFailureReason>, Self::VerifyTxError>;

    // FIXME: Change type to encode the ordering
    /// Send a batch, where the first index contains the first transaction. 
    async fn send_batch(
        &self,
        txs: Vec<Arc<ProvenTransaction>>,
    ) -> Result<(), Self::ProduceBatchError>;
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

/// Creates and runs the transaction queue task
pub async fn run_tx_queue_task<HandleIn, HandleOut>(
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

/// The transaction queue task
#[derive(Clone)]
struct TxQueue<HandleIn, HandleOut>
where
    HandleIn: TxQueueHandleIn,
    HandleOut: TxQueueHandleOut,
{
    ready_queue: ReadyQueue,
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

    /// Start the task
    pub async fn run(self) {
        let tx_queue = Arc::new(self);
        let timer_task_handle = start_timer_task(
            tx_queue.ready_queue.clone(),
            tx_queue.handle_out.clone(),
            tx_queue.options.tx_max_time_in_queue,
            tx_queue.options.batch_size,
        );

        loop {
            // Handle new transaction coming in
            let proven_tx =
                tx_queue.handle_in.read_transaction().await.expect("Failed to read transaction");
            let tx_queue = tx_queue.clone();
            let timer_task_handle = timer_task_handle.clone();

            tokio::spawn(async move {
                tx_queue.on_read_transaction(proven_tx, timer_task_handle).await
            });
        }
    }

    // HELPERS
    // --------------------------------------------------------------------------------------------

    async fn on_read_transaction(
        self: Arc<TxQueue<HandleIn, HandleOut>>,
        proven_tx: ProvenTransaction,
        timer_task_handle: TimerTaskHandle,
    ) {
        let proven_tx = Arc::new(proven_tx);

        let verification_result = self
            .handle_out
            .verify_transaction(proven_tx.clone())
            .await
            .expect("Failed to verify transaction");

        if let Err(_failure_reason) = verification_result {
            // TODO: Log failure properly
            return;
        }

        // Transaction verification succeeded. It is safe to add transaction to queue.
        let mut locked_ready_queue = self.ready_queue.lock().await;

        if locked_ready_queue.is_empty() {
            timer_task_handle.start_timer();
        }

        locked_ready_queue.push(proven_tx);

        if locked_ready_queue.len() >= self.options.batch_size {
            // FIXME: Dropping the ready queue here means that 2 tasks could send a batch, where the
            // first batch will contain `batch_size` transactions, and the second would contain only
            // 1 transaction. This is low-risk and has little-to-no impact if it occurs.
            drop(locked_ready_queue);

            // We are sending a batch, so reset the timer
            timer_task_handle.stop_timer();

            let ready_queue = self.ready_queue.clone();
            let handle_out = self.handle_out.clone();

            tokio::spawn(send_batch(ready_queue, handle_out, self.options.batch_size));
        }
    }
}

// TIMER TASK
// ================================================================================================

fn start_timer_task<HandleOut: TxQueueHandleOut>(
    ready_queue: ReadyQueue,
    handle_out: Arc<HandleOut>,
    tx_max_time_in_queue: Duration,
    batch_size: usize,
) -> TimerTaskHandle {
    let (timer_task, handle) =
        TimerTask::new(ready_queue, handle_out, tx_max_time_in_queue, batch_size);

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
struct TimerTask<HandleOut: TxQueueHandleOut> {
    ready_queue: ReadyQueue,
    receiver: UnboundedReceiver<TimerMessage>,
    handle_out: Arc<HandleOut>,
    tx_max_time_in_queue: Duration,
    batch_size: usize,
}

impl<HandleOut> TimerTask<HandleOut>
where
    HandleOut: TxQueueHandleOut,
{
    pub fn new(
        ready_queue: ReadyQueue,
        handle_out: Arc<HandleOut>,
        tx_max_time_in_queue: Duration,
        batch_size: usize,
    ) -> (Self, TimerTaskHandle) {
        let (sender, receiver) = unbounded_channel();

        (
            Self {
                ready_queue,
                receiver,
                handle_out,
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
                    tokio::spawn(send_batch(self.ready_queue.clone(), self.handle_out.clone(), self.batch_size));
                    sleep_duration = Duration::MAX;
                }
            }
        }
    }
}

// HELPERS
// ================================================================================================

/// Drains the queue and sends the batch. This task is responsible for ensuring that the batch is
/// successfully sent, whether this requires retries, or any other strategy.
async fn send_batch<HandleOut: TxQueueHandleOut>(
    ready_queue: ReadyQueue,
    handle_out: Arc<HandleOut>,
    batch_size: usize,
) {
    let txs_in_batch: Vec<Arc<ProvenTransaction>> = {
        // drain `batch_size` txs from the queue and release the lock.
        let mut locked_ready_queue = ready_queue.lock().await;

        let num_to_drain = min(batch_size, locked_ready_queue.len());
        locked_ready_queue.drain(..num_to_drain).collect()
    };

    if txs_in_batch.is_empty() {
        return;
    }

    // Panic for now if the send fails. In the future, we might want a more sophisticated strategy,
    // such as retrying, or something else.
    handle_out.send_batch(txs_in_batch).await.expect("Failed to send batch");
}
