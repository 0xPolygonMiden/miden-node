use super::*;
use crate::{errors::BuildBlockError, test_utils::DummyProvenTxGenerator, TransactionBatch};

// STRUCTS
// ================================================================================================

#[derive(Default)]
struct BlockBuilderSuccess {
    batch_groups: SharedRwVec<Vec<TransactionBatch>>,
    num_empty_batches_received: Arc<RwLock<usize>>,
}

#[async_trait]
impl BlockBuilder for BlockBuilderSuccess {
    async fn build_block(
        &self,
        batches: &[TransactionBatch],
    ) -> Result<(), BuildBlockError> {
        if batches.is_empty() {
            *self.num_empty_batches_received.write().await += 1;
        } else {
            self.batch_groups.write().await.push(batches.to_vec());
        }

        Ok(())
    }
}

#[derive(Default)]
struct BlockBuilderFailure;

#[async_trait]
impl BlockBuilder for BlockBuilderFailure {
    async fn build_block(
        &self,
        _batches: &[TransactionBatch],
    ) -> Result<(), BuildBlockError> {
        Err(BuildBlockError::TooManyBatchesInBlock(0))
    }
}

// TESTS
// ================================================================================================

/// Tests that the number of batches in a block doesn't exceed `max_batches_per_block`
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_block_size_doesnt_exceed_limit() {
    let block_frequency = Duration::from_millis(20);
    let max_batches_per_block = 2;

    let block_builder = Arc::new(BlockBuilderSuccess::default());

    let batch_builder = Arc::new(DefaultBatchBuilder::new(
        block_builder.clone(),
        DefaultBatchBuilderOptions {
            block_frequency,
            max_batches_per_block,
        },
    ));

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

/// Tests that `BlockBuilder::build_block()` is still called when there are no transactions
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_build_block_called_when_no_batches() {
    let block_frequency = Duration::from_millis(20);
    let max_batches_per_block = 2;

    let block_builder = Arc::new(BlockBuilderSuccess::default());

    let batch_builder = Arc::new(DefaultBatchBuilder::new(
        block_builder.clone(),
        DefaultBatchBuilderOptions {
            block_frequency,
            max_batches_per_block,
        },
    ));

    // start batch builder
    tokio::spawn(batch_builder.run());

    // Wait for at least 1 block to be produced
    time::sleep(block_frequency * 2).await;

    // Ensure the block builder received at least 1 empty batch Note: we check `> 0` instead of an
    // exact number to make the test flaky in case timings change in the implementation
    assert!(*block_builder.num_empty_batches_received.read().await > 0);
}

/// Tests that if `BlockBuilder::build_block()` fails, then batches are added back on the queue
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_batches_added_back_to_queue_on_block_build_failure() {
    let block_frequency = Duration::from_millis(20);
    let max_batches_per_block = 2;

    let block_builder = Arc::new(BlockBuilderFailure);

    let batch_builder = Arc::new(DefaultBatchBuilder::new(
        block_builder.clone(),
        DefaultBatchBuilderOptions {
            block_frequency,
            max_batches_per_block,
        },
    ));

    let internal_ready_batches = batch_builder.ready_batches.clone();

    // Add 3 batches in internal queue
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

    // Wait for 2 blocks to failed to be produced
    time::sleep(block_frequency * 2 + (block_frequency / 2)).await;

    // Ensure the transaction batches are all still on the queue
    assert_eq!(internal_ready_batches.read().await.len(), 3);
}

// HELPERS
// ================================================================================================

fn dummy_tx_batch(
    tx_gen: &DummyProvenTxGenerator,
    num_txs_in_batch: usize,
) -> TransactionBatch {
    let txs: Vec<_> = (0..num_txs_in_batch).map(|_| tx_gen.dummy_proven_tx()).collect();
    TransactionBatch::new(txs).unwrap()
}
