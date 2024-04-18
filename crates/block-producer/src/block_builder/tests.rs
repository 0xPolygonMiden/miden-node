// block builder tests (higher level)
// `apply_block()` is called

use std::sync::Arc;

use miden_objects::{
    accounts::{AccountId, ACCOUNT_ID_OFF_CHAIN_SENDER},
    Digest, Felt,
};

use crate::{
    batch_builder::TransactionBatch,
    block_builder::{BlockBuilder, BuildBlockError, DefaultBlockBuilder},
    test_utils::{MockProvenTxBuilder, MockStoreFailure, MockStoreSuccessBuilder},
};

/// Tests that `build_block()` succeeds when the transaction batches are not empty
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_apply_block_called_nonempty_batches() {
    let account_id = AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER));
    let account_initial_hash: Digest =
        [Felt::new(1u64), Felt::new(1u64), Felt::new(1u64), Felt::new(1u64)].into();
    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(std::iter::once((account_id, account_initial_hash)))
            .build(),
    );

    let block_builder = DefaultBlockBuilder::new(store.clone(), store.clone());

    let batches: Vec<TransactionBatch> = {
        let batch_1 = {
            let tx = MockProvenTxBuilder::with_account(
                account_id,
                account_initial_hash,
                [Felt::new(2u64), Felt::new(2u64), Felt::new(2u64), Felt::new(2u64)].into(),
            )
            .build();

            TransactionBatch::new(vec![tx]).unwrap()
        };

        vec![batch_1]
    };
    block_builder.build_block(&batches).await.unwrap();

    // Ensure that the store's `apply_block()` was called
    assert_eq!(*store.num_apply_block_called.read().await, 1);
}

/// Tests that `build_block()` succeeds when the transaction batches are empty
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_apply_block_called_empty_batches() {
    let account_id = AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER));
    let account_hash: Digest =
        [Felt::new(1u64), Felt::new(1u64), Felt::new(1u64), Felt::new(1u64)].into();
    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(std::iter::once((account_id, account_hash))).build(),
    );

    let block_builder = DefaultBlockBuilder::new(store.clone(), store.clone());

    block_builder.build_block(&Vec::new()).await.unwrap();

    // Ensure that the store's `apply_block()` was called
    assert_eq!(*store.num_apply_block_called.read().await, 1);
}

/// Tests that `build_block()` fails when `get_block_inputs()` fails
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_build_block_failure() {
    let store = Arc::new(MockStoreFailure);

    let block_builder = DefaultBlockBuilder::new(store.clone(), store.clone());

    let result = block_builder.build_block(&Vec::new()).await;

    // Ensure that the store's `apply_block()` was called
    assert!(matches!(result, Err(BuildBlockError::GetBlockInputsFailed(_))));
}
