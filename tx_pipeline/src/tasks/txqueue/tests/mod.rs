// TODO: to test
// 3. timer is reset when batch is sent from timer
// 4. timer is reset when batch is sent from batch
// 6. Confirm that if batch size is 5, and 6 txs in queue, only 5 are removed from queue
mod batch_sent;

use super::*;
use crate::test_utils::DummyProvenTxGenerator;
use std::{convert::Infallible, time::Duration};
use tokio::{sync::RwLock, time};

/// calls `read_transaction()` a given number of times at a fixed interval
pub struct HandleInFixedInterval {
    interval_duration: Duration,
    num_txs_to_send: usize,
    txs_sent_count: Arc<RwLock<usize>>,
    proven_tx_gen: DummyProvenTxGenerator,
}

impl HandleInFixedInterval {
    pub fn new(
        interval_duration: Duration,
        num_txs_to_send: usize,
    ) -> Self {
        Self {
            interval_duration,
            num_txs_to_send,
            txs_sent_count: Arc::new(RwLock::new(0)),
            proven_tx_gen: DummyProvenTxGenerator::new(),
        }
    }
}

#[async_trait]
impl TxQueueHandleIn for HandleInFixedInterval {
    type ReadTxError = Infallible;

    async fn read_transaction(&self) -> Result<ProvenTransaction, Self::ReadTxError> {
        // if we already sent the right amount of txs, sleep forever
        if *self.txs_sent_count.read().await >= self.num_txs_to_send {
            // sleep forever
            time::sleep(Duration::MAX).await;
            panic!("woke up from forever sleep?");
        }

        *self.txs_sent_count.write().await += 1;

        // sleep for the pre-determined time
        time::sleep(self.interval_duration).await;

        Ok(self.proven_tx_gen.dummy_proven_tx())
    }
}

/// calls `read_transaction()` a given number of times at a variable interval
pub struct HandleInVariableInterval {
    /// Encodes how long to wait before sending the ith transaction.
    /// Thus, we send `interval_durations.len()` transactions.
    interval_durations: Vec<Duration>,
    txs_sent_count: Arc<RwLock<usize>>,
    proven_tx_gen: DummyProvenTxGenerator,
}

impl HandleInVariableInterval {
    pub fn new(interval_durations: Vec<Duration>) -> Self {
        Self {
            interval_durations,
            txs_sent_count: Arc::new(RwLock::new(0)),
            proven_tx_gen: DummyProvenTxGenerator::new(),
        }
    }
}

#[async_trait]
impl TxQueueHandleIn for HandleInVariableInterval {
    type ReadTxError = Infallible;

    async fn read_transaction(&self) -> Result<ProvenTransaction, Self::ReadTxError> {
        let txs_sent_count = *self.txs_sent_count.read().await;

        // if we already sent the right amount of txs, sleep forever
        if txs_sent_count >= self.interval_durations.len() {
            // sleep forever
            time::sleep(Duration::MAX).await;
            panic!("woke up from forever sleep?");
        }

        // sleep for the pre-determined time
        time::sleep(self.interval_durations[txs_sent_count]).await;

        *self.txs_sent_count.write().await += 1;

        Ok(self.proven_tx_gen.dummy_proven_tx())
    }
}

/// All transactions verify successfully. Records all sent batches.
#[derive(Clone)]
pub struct HandleOutDefault {
    // TODO: Simplify type
    pub batches: Arc<RwLock<Vec<Vec<Arc<ProvenTransaction>>>>>,
}

impl HandleOutDefault {
    pub fn new() -> Self {
        Self {
            batches: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl TxQueueHandleOut for HandleOutDefault {
    type VerifyTxError = Infallible;
    type TxVerificationFailureReason = ();
    type ProduceBatchError = Infallible;

    async fn verify_transaction(
        &self,
        _tx: Arc<ProvenTransaction>,
    ) -> Result<Result<(), Self::TxVerificationFailureReason>, Self::VerifyTxError> {
        Ok(Ok(()))
    }

    async fn send_batch(
        &self,
        txs: Vec<Arc<ProvenTransaction>>,
    ) -> Result<(), Self::ProduceBatchError> {
        self.batches.write().await.push(txs);

        Ok(())
    }
}

/// All transactions fail verification. Records all sent batches.
#[derive(Clone)]
pub struct HandleOutFailVerification {
    // TODO: Simplify type
    pub batches: Arc<RwLock<Vec<Vec<Arc<ProvenTransaction>>>>>,
}

impl HandleOutFailVerification {
    pub fn new() -> Self {
        Self {
            batches: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl TxQueueHandleOut for HandleOutFailVerification {
    type VerifyTxError = ();
    type TxVerificationFailureReason = ();
    type ProduceBatchError = Infallible;

    async fn verify_transaction(
        &self,
        _tx: Arc<ProvenTransaction>,
    ) -> Result<Result<(), Self::TxVerificationFailureReason>, Self::VerifyTxError> {
        Ok(Err(()))
    }

    async fn send_batch(
        &self,
        txs: Vec<Arc<ProvenTransaction>>,
    ) -> Result<(), Self::ProduceBatchError> {
        self.batches.write().await.push(txs);

        Ok(())
    }
}
