use tokio::time;

use super::*;
use crate::test_utils::DummyProvenTxGenerator;

pub struct ReadTxClientFixedInterval {
    read_tx_client: RpcClient<ProvenTransaction, ()>,
    interval_duration: Duration,
    num_txs_to_send: usize,
    proven_tx_gen: DummyProvenTxGenerator,
}

impl ReadTxClientFixedInterval {
    pub fn new(
        read_tx_client: RpcClient<ProvenTransaction, ()>,
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
    }
}

pub struct ReadTxClientVariableInterval {
    read_tx_client: RpcClient<ProvenTransaction, ()>,
    /// Encodes how long to wait before sending the ith transaction. Thus, we send
    /// `interval_durations.len()` transactions.
    interval_durations: Vec<Duration>,
    proven_tx_gen: DummyProvenTxGenerator,
}

impl ReadTxClientVariableInterval {
    pub fn new(
        read_tx_client: RpcClient<ProvenTransaction, ()>,
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
    }
}
