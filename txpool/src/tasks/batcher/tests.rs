use miden_air::{ExecutionProof, HashFunction};
use miden_mock::constants::ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_ON_CHAIN;
use miden_objects::{accounts::AccountId, Digest};
use tokio::time::sleep;
use winterfell::StarkProof;

use crate::test_utils::dummy_stark_proof;

use super::*;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

const NUM_RECEIVE_TXS: u64 = 3;
const RECEIVE_INTERVAL_MS: u64 = 30;
const FUDGE_FACTOR_MS: u64 = 15;
// Sleep for all transactions to be sent, plus a fudge factor
const NOTIFICATION_SLEEP_MS: u64 = NUM_RECEIVE_TXS * RECEIVE_INTERVAL_MS + FUDGE_FACTOR_MS;

/// A task handle that sends txs and notifications to the batcher task based on a timer
#[derive(Clone)]
struct MockTimerBatcherTaskHandle {
    // captures the batch that was sent by the batcher task
    sent_batch: Arc<RwLock<TxBatch>>,
    received_txs: Arc<RwLock<u64>>,
    stark_proof: StarkProof,
}

impl Default for MockTimerBatcherTaskHandle {
    fn default() -> Self {
        Self {
            sent_batch: Default::default(),
            received_txs: Default::default(),
            stark_proof: dummy_stark_proof(),
        }
    }
}

impl MockTimerBatcherTaskHandle {
    fn dummy_proven_tx(&self) -> ProvenTransaction {
        ProvenTransaction::new(
            AccountId::try_from(ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_ON_CHAIN).unwrap(),
            Digest::default(),
            Digest::default(),
            Vec::new(),
            Vec::new(),
            None,
            Digest::default(),
            ExecutionProof::new(self.stark_proof.clone(), HashFunction::Blake3_192),
        )
    }
}

#[async_trait]
impl BatcherTaskHandle for MockTimerBatcherTaskHandle {
    type SendError = ();
    type ReceiveError = ();

    async fn wait_for_send_batch_notification(&self) {
        // Sleep for all transactions to be sent, plus an extra one (fudge factor)
        let sleep_duration = Duration::from_millis(NOTIFICATION_SLEEP_MS);

        sleep(sleep_duration).await;
    }

    async fn receive_tx(&self) -> Result<ProvenTransaction, Self::ReceiveError> {
        let num_txs_received = *self.received_txs.read().unwrap();
        if num_txs_received >= NUM_RECEIVE_TXS {
            // We already sent all our txs, so wait "forever"
            sleep(Duration::from_secs(60)).await;
            panic!("Sent all txs and waited for 1 minute");
        } else {
            *self.received_txs.write().unwrap() += 1;

            sleep(Duration::from_millis(RECEIVE_INTERVAL_MS)).await;

            Ok(self.dummy_proven_tx())
        }
    }

    async fn send_batch(
        &self,
        txs: TxBatch,
    ) -> Result<(), Self::SendError> {
        *self.sent_batch.write().unwrap() = txs;

        Ok(())
    }
}

#[tokio::test]
async fn test_notification() {
    let handle = MockTimerBatcherTaskHandle::default();

    tokio::spawn(batcher_task(handle.clone()));

    // wait for the notification
    sleep(Duration::from_millis(NOTIFICATION_SLEEP_MS + FUDGE_FACTOR_MS)).await;

    assert_eq!(*handle.received_txs.read().unwrap(), NUM_RECEIVE_TXS);
}
