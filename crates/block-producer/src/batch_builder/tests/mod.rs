use std::iter;

use assert_matches::assert_matches;
use miden_objects::{crypto::merkle::Mmr, Digest};
use tokio::sync::RwLock;

use super::*;
use crate::{
    block_builder::DefaultBlockBuilder,
    errors::BuildBlockError,
    test_utils::{
        note::mock_note, MockPrivateAccount, MockProvenTxBuilder, MockStoreSuccessBuilder,
    },
};
// STRUCTS
// ================================================================================================

#[derive(Default)]
struct BlockBuilderSuccess {
    batch_groups: SharedRwVec<Vec<TransactionBatch>>,
    num_empty_batches_received: Arc<RwLock<usize>>,
}

#[async_trait]
impl BlockBuilder for BlockBuilderSuccess {
    async fn build_block(&self, batches: &[TransactionBatch]) -> Result<(), BuildBlockError> {
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
    async fn build_block(&self, _batches: &[TransactionBatch]) -> Result<(), BuildBlockError> {
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

    let store = Arc::new(MockStoreSuccessBuilder::from_accounts(iter::empty()).build());
    let block_builder = Arc::new(BlockBuilderSuccess::default());

    let batch_builder = Arc::new(DefaultBatchBuilder::new(
        store,
        block_builder.clone(),
        DefaultBatchBuilderOptions { block_frequency, max_batches_per_block },
    ));

    // Add 3 batches in internal queue (remember: 2 batches/block)
    {
        let mut batch_group =
            vec![dummy_tx_batch(0, 2), dummy_tx_batch(10, 2), dummy_tx_batch(20, 2)];

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

    let store = Arc::new(MockStoreSuccessBuilder::from_accounts(iter::empty()).build());
    let block_builder = Arc::new(BlockBuilderSuccess::default());

    let batch_builder = Arc::new(DefaultBatchBuilder::new(
        store,
        block_builder.clone(),
        DefaultBatchBuilderOptions { block_frequency, max_batches_per_block },
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

    let store = Arc::new(MockStoreSuccessBuilder::from_accounts(iter::empty()).build());
    let block_builder = Arc::new(BlockBuilderFailure);

    let batch_builder = Arc::new(DefaultBatchBuilder::new(
        store,
        block_builder.clone(),
        DefaultBatchBuilderOptions { block_frequency, max_batches_per_block },
    ));

    let internal_ready_batches = batch_builder.ready_batches.clone();

    // Add 3 batches in internal queue
    {
        let mut batch_group =
            vec![dummy_tx_batch(0, 2), dummy_tx_batch(10, 2), dummy_tx_batch(20, 2)];

        batch_builder.ready_batches.write().await.append(&mut batch_group);
    }

    // start batch builder
    tokio::spawn(batch_builder.run());

    // Wait for 2 blocks to failed to be produced
    time::sleep(block_frequency * 2 + (block_frequency / 2)).await;

    // Ensure the transaction batches are all still on the queue
    assert_eq!(internal_ready_batches.read().await.len(), 3);
}

#[tokio::test]
async fn test_batch_builder_find_dangling_notes() {
    let store = Arc::new(MockStoreSuccessBuilder::from_accounts(iter::empty()).build());
    let block_builder = Arc::new(BlockBuilderSuccess::default());

    let batch_builder = Arc::new(DefaultBatchBuilder::new(
        store,
        block_builder,
        DefaultBatchBuilderOptions {
            block_frequency: Duration::from_millis(20),
            max_batches_per_block: 2,
        },
    ));

    // An account with 5 states so that we can simulate running 2 transactions against it.
    let account = MockPrivateAccount::<3>::from(1);

    let note_1 = mock_note(1);
    let note_2 = mock_note(2);
    let tx1 = MockProvenTxBuilder::with_account(account.id, account.states[0], account.states[1])
        .output_notes(vec![OutputNote::Full(note_1.clone())])
        .build();
    let tx2 = MockProvenTxBuilder::with_account(account.id, account.states[1], account.states[2])
        .unauthenticated_notes(vec![note_1.clone()])
        .output_notes(vec![OutputNote::Full(note_2.clone())])
        .build();

    let txs = vec![tx1, tx2];

    let dangling_notes = batch_builder.find_dangling_notes(&txs).await;
    assert_eq!(dangling_notes, vec![], "Note must be presented in the same batch");

    batch_builder.build_batch(txs.clone()).await.unwrap();

    let dangling_notes = batch_builder.find_dangling_notes(&txs).await;
    assert_eq!(dangling_notes, vec![], "Note must be presented in the same batch");

    let note_3 = mock_note(3);

    let tx1 = MockProvenTxBuilder::with_account(account.id, account.states[0], account.states[1])
        .unauthenticated_notes(vec![note_2.clone()])
        .build();
    let tx2 = MockProvenTxBuilder::with_account(account.id, account.states[1], account.states[2])
        .unauthenticated_notes(vec![note_3.clone()])
        .build();

    let txs = vec![tx1, tx2];

    let dangling_notes = batch_builder.find_dangling_notes(&txs).await;
    assert_eq!(
        dangling_notes,
        vec![note_3.id()],
        "Only one dangling node must be found before block is built"
    );

    batch_builder.try_build_block().await;

    let dangling_notes = batch_builder.find_dangling_notes(&txs).await;
    assert_eq!(
        dangling_notes,
        vec![note_2.id(), note_3.id()],
        "Two dangling notes must be found after block is built"
    );
}

#[tokio::test]
async fn test_block_builder_no_missing_notes() {
    let account_1: MockPrivateAccount<3> = MockPrivateAccount::from(1);
    let account_2: MockPrivateAccount<3> = MockPrivateAccount::from(2);
    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(
            [account_1, account_2].iter().map(|account| (account.id, account.states[0])),
        )
        .build(),
    );
    let block_builder = Arc::new(DefaultBlockBuilder::new(Arc::clone(&store), Arc::clone(&store)));
    let batch_builder = Arc::new(DefaultBatchBuilder::new(
        store,
        Arc::clone(&block_builder),
        DefaultBatchBuilderOptions {
            block_frequency: Duration::from_millis(20),
            max_batches_per_block: 2,
        },
    ));

    let note_1 = mock_note(1);
    let note_2 = mock_note(2);

    let tx1 = MockProvenTxBuilder::with_account_index(1)
        .output_notes(vec![OutputNote::Full(note_1.clone())])
        .build();

    let tx2 = MockProvenTxBuilder::with_account_index(2)
        .unauthenticated_notes(vec![note_1.clone()])
        .output_notes(vec![OutputNote::Full(note_2.clone())])
        .build();

    let txs = vec![tx1, tx2];

    batch_builder.build_batch(txs.clone()).await.unwrap();

    let build_block_result = batch_builder
        .block_builder
        .build_block(&batch_builder.ready_batches.read().await)
        .await;
    assert_matches!(build_block_result, Ok(()));
}

#[tokio::test]
async fn test_block_builder_fails_if_notes_are_missing() {
    let accounts: Vec<_> = (1..=4).map(MockPrivateAccount::<3>::from).collect();
    let notes: Vec<_> = (1..=6).map(mock_note).collect();
    // We require mmr for the note authentication to succeed.
    //
    // We also need two blocks worth of mmr because the mock store skips genesis.
    let mut mmr = Mmr::new();
    mmr.add(Digest::new([1u32.into(), 2u32.into(), 3u32.into(), 4u32.into()]));
    mmr.add(Digest::new([1u32.into(), 2u32.into(), 3u32.into(), 4u32.into()]));

    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(
            accounts.iter().map(|account| (account.id, account.states[0])),
        )
        .initial_notes([vec![OutputNote::Full(notes[0].clone())]].iter())
        .initial_chain_mmr(mmr)
        .build(),
    );
    let block_builder = Arc::new(DefaultBlockBuilder::new(Arc::clone(&store), Arc::clone(&store)));
    let batch_builder = Arc::new(DefaultBatchBuilder::new(
        store,
        Arc::clone(&block_builder),
        DefaultBatchBuilderOptions {
            block_frequency: Duration::from_millis(20),
            max_batches_per_block: 2,
        },
    ));

    let tx1 = MockProvenTxBuilder::with_account_index(1)
        .output_notes(vec![OutputNote::Full(notes[1].clone())])
        .build();

    let tx2 = MockProvenTxBuilder::with_account_index(2)
        .unauthenticated_notes(vec![notes[0].clone()])
        .output_notes(vec![OutputNote::Full(notes[2].clone()), OutputNote::Full(notes[3].clone())])
        .build();

    let tx3 = MockProvenTxBuilder::with_account_index(3)
        .unauthenticated_notes(notes.iter().skip(1).cloned().collect())
        .build();

    let txs = vec![tx1, tx2, tx3];

    let batch = TransactionBatch::new(txs.clone(), Default::default()).unwrap();
    let build_block_result = batch_builder.block_builder.build_block(&[batch]).await;

    let mut expected_missing_notes = vec![notes[4].id(), notes[5].id()];
    expected_missing_notes.sort();

    assert_matches!(
        build_block_result,
        Err(BuildBlockError::UnauthenticatedNotesNotFound(actual_missing_notes)) => {
            assert_eq!(actual_missing_notes, expected_missing_notes);
        }
    );
}

// HELPERS
// ================================================================================================

fn dummy_tx_batch(starting_account_index: u32, num_txs_in_batch: usize) -> TransactionBatch {
    let txs = (0..num_txs_in_batch)
        .map(|index| {
            MockProvenTxBuilder::with_account_index(starting_account_index + index as u32).build()
        })
        .collect();
    TransactionBatch::new(txs, Default::default()).unwrap()
}
