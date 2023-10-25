use super::*;
use crate::{block_builder::BuildBlockError, test_utils::DummyProvenTxGenerator};

// STRUCTS
// ================================================================================================

/// Batches that would be used to create a block, except we don't actually build a block in these
/// tests
type BatchGroup = Vec<Arc<TransactionBatch>>;

#[derive(Default)]
struct BlockBuilderSuccess {
    batch_groups: SharedRwVec<BatchGroup>,
}

#[async_trait]
impl BlockBuilder for BlockBuilderSuccess {
    async fn build_block(
        &self,
        batch: Vec<Arc<TransactionBatch>>,
    ) -> Result<(), BuildBlockError> {
        self.batch_groups.write().await.push(batch);

        Ok(())
    }
}

#[derive(Default)]
struct BlockBuilderFailure;

#[async_trait]
impl BlockBuilder for BlockBuilderFailure {
    async fn build_block(
        &self,
        _batch: Vec<Arc<TransactionBatch>>,
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
) -> Arc<TransactionBatch> {
    let txs: Vec<_> = (0..num_txs_in_batch)
        .into_iter()
        .map(|_| Arc::new(tx_gen.dummy_proven_tx()))
        .collect();

    Arc::new(TransactionBatch::new(txs))
}
