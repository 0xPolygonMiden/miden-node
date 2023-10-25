use super::*;
use crate::{block_builder::BuildBlockError, test_utils::DummyProvenTxGenerator, SharedTxBatch};

// STRUCTS
// ================================================================================================

#[derive(Default)]
struct BlockBuilderSuccess {
    batch_groups: SharedRwVec<Vec<SharedTxBatch>>,
    num_empty_batches_received: Arc<RwLock<usize>>,
}

#[async_trait]
impl BlockBuilder for BlockBuilderSuccess {
    async fn build_block(
        &self,
        batches: Option<Vec<SharedTxBatch>>,
    ) -> Result<(), BuildBlockError> {
        match batches {
            Some(batches) => {
                self.batch_groups.write().await.push(batches);
            },
            None => {
                *self.num_empty_batches_received.write().await += 1;
            },
        };

        Ok(())
    }
}

#[derive(Default)]
struct BlockBuilderFailure;

#[async_trait]
impl BlockBuilder for BlockBuilderFailure {
    async fn build_block(
        &self,
        _batches: Option<Vec<SharedTxBatch>>,
    ) -> Result<(), BuildBlockError> {
        Err(BuildBlockError::Dummy)
    }
}

// TESTS
// ================================================================================================

/// Tests that the number of batches in a block doesn't exceed `max_batches_per_block`
#[tokio::test]
async fn test_block_size_doesnt_exceed_limit() {
    let block_frequency = Duration::from_millis(20);
    let max_batches_per_block = 2;

    let block_builder = Arc::new(BlockBuilderSuccess::default());

    let batch_builder = DefaultBatchBuilder::new(
        block_builder.clone(),
        DefaultBatchBuilderOptions {
            block_frequency,
            max_batches_per_block,
        },
    );

    // Add 3 batches in internal queue (remember: 2 batches/block)
    {
        let tx_gen = DummyProvenTxGenerator::new();

        let mut batch_group = vec![
            dummy_tx_batch(&tx_gen, 2),
            dummy_tx_batch(&tx_gen, 2),
            dummy_tx_batch(&tx_gen, 2),
        ];

        batch_builder.ready_batches.write().await.append(&mut batch_group);
    }

    // start batch builder
    tokio::spawn(batch_builder.run());

    // Wait for 2 blocks to be produced
    time::sleep(block_frequency * 3).await;

    // Ensure the block builder received 2 batches of the expected size
    {
        let batch_groups = block_builder.batch_groups.read().await;

        assert_eq!(batch_groups.len(), 2);
        assert_eq!(batch_groups[0].len(), max_batches_per_block);
        assert_eq!(batch_groups[1].len(), 1);
    }
}

// HELPERS
// ================================================================================================

fn dummy_tx_batch(
    tx_gen: &DummyProvenTxGenerator,
    num_txs_in_batch: usize,
) -> SharedTxBatch {
    let txs: Vec<_> = (0..num_txs_in_batch)
        .into_iter()
        .map(|_| Arc::new(tx_gen.dummy_proven_tx()))
        .collect();

    Arc::new(TransactionBatch::new(txs))
}
