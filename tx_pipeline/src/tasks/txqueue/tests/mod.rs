// TODO: to test
// 0. if tx validation fails, it is not added to the queue
// 1. if batch size not full, timer procs and sends batch
// 2. when batch size is full (before timer), batch is sent
// 3. timer is reset when batch is sent from timer
// 4. timer is reset when batch is sent from batch

use super::*;
use crate::test_utils::DummyProvenTxGenerator;
use std::{convert::Infallible, time::Duration};
use tokio::{sync::RwLock, time};

/// calls `read_transaction()` a given number of times at a fixed interval
pub struct HandleInInterval {
    interval_duration: Duration,
    num_txs_to_send: usize,
    txs_sent_count: Arc<RwLock<usize>>,
    proven_tx_gen: DummyProvenTxGenerator,
}

impl HandleInInterval {
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
impl TxQueueHandleIn for HandleInInterval {
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

/// All transactions verify successfully. Records all sent batches.
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
