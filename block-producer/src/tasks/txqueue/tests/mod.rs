mod batch_sent;

use tokio::time;

use super::*;
use crate::test_utils::DummyProvenTxGenerator;

// CLIENT IMPLS
// ================================================================================================

/// Sends a transaction to the server at a fixed interval. The first transaction is sent at t=0.
pub struct ReadTxClientFixedInterval {
    read_tx_client: MessageSender<ProvenTransaction, ()>,
    interval_duration: Duration,
    num_txs_to_send: usize,
    proven_tx_gen: DummyProvenTxGenerator,
}

impl ReadTxClientFixedInterval {
    pub fn new(
        read_tx_client: MessageSender<ProvenTransaction, ()>,
        interval_duration: Duration,
        num_txs_to_send: usize,
    ) -> Self {
        Self {
            read_tx_client,
            interval_duration,
            num_txs_to_send,
            proven_tx_gen: DummyProvenTxGenerator::new(),
        }
    }

    pub async fn run(self) {
        let mut interval = time::interval(self.interval_duration);

        for _ in 0..self.num_txs_to_send {
            self.read_tx_client
                .call(self.proven_tx_gen.dummy_proven_tx())
                .unwrap()
                .await
                .unwrap();

            interval.tick().await;
        }

        // Hang forever
        sleep(Duration::MAX).await;
    }
}

pub struct ReadTxClientVariableInterval {
    read_tx_client: MessageSender<ProvenTransaction, ()>,
    /// Encodes how long to wait before sending the ith transaction. Thus, we send
    /// `interval_durations.len()` transactions.
    interval_durations: Vec<Duration>,
    proven_tx_gen: DummyProvenTxGenerator,
}

impl ReadTxClientVariableInterval {
    pub fn new(
        read_tx_client: MessageSender<ProvenTransaction, ()>,
        interval_durations: Vec<Duration>,
    ) -> Self {
        Self {
            read_tx_client,
            interval_durations,
            proven_tx_gen: DummyProvenTxGenerator::new(),
        }
    }

    pub async fn run(self) {
        for duration in self.interval_durations {
            time::sleep(duration).await;

            self.read_tx_client
                .call(self.proven_tx_gen.dummy_proven_tx())
                .unwrap()
                .await
                .unwrap();
        }

        // Hang forever
        sleep(Duration::MAX).await;
    }
}

// SERVER IMPLS
// ================================================================================================

/// All transactions succeed verification.
pub struct VerifyTxRpcSuccess;

#[async_trait]
impl MessageHandler<SharedProvenTx, Result<(), VerifyTxError>> for VerifyTxRpcSuccess {
    async fn handle_message(
        self: Arc<Self>,
        _proven_tx: SharedProvenTx,
    ) -> Result<(), VerifyTxError> {
        Ok(())
    }
}

/// All transactions fail verification.
pub struct VerifyTxRpcFailure;

#[async_trait]
impl MessageHandler<SharedProvenTx, Result<(), VerifyTxError>> for VerifyTxRpcFailure {
    async fn handle_message(
        self: Arc<Self>,
        _proven_tx: SharedProvenTx,
    ) -> Result<(), VerifyTxError> {
        Err(VerifyTxError::Dummy)
    }
}

pub struct SendTxsDefaultServerImpl {
    pub batches: SharedMutVec<Vec<SharedProvenTx>>,
}

impl SendTxsDefaultServerImpl {
    pub fn new() -> Self {
        Self {
            batches: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl MessageHandler<Vec<SharedProvenTx>, ()> for SendTxsDefaultServerImpl {
    async fn handle_message(
        self: Arc<Self>,
        proven_txs: Vec<SharedProvenTx>,
    ) {
        self.batches.lock().await.push(proven_txs);
    }
}
