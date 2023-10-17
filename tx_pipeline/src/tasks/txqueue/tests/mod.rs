// TODO: to test
// 0. if tx validation fails, it is not added to the queue
// 1. if batch size not full, timer procs and sends batch
// 2. when batch size is full (before timer), batch is sent
// 3. timer is reset when batch is sent from timer
// 4. timer is reset when batch is sent from batch

use crate::test_utils::DummyProvenTxGenerator;

use super::*;
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
