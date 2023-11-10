use std::collections::BTreeSet;

// block builder tests (higher level)
// 1. `apply_block()` is called
use super::*;

use miden_air::Felt;

use crate::test_utils::{MockStoreFailure, MockStoreSuccess};

/// Tests that `build_block()` succeeds when the transaction batches are empty
#[tokio::test]
async fn test_apply_block_called_empty_batches() {
    let account_id = unsafe { AccountId::new_unchecked(42u64.into()) };
    let account_hash: Digest =
        [Felt::from(1u64), Felt::from(1u64), Felt::from(1u64), Felt::from(1u64)].into();
    let store = Arc::new(MockStoreSuccess::new(
        std::iter::once((account_id, account_hash)),
        BTreeSet::new(),
    ));

    let block_builder = DefaultBlockBuilder::new(store.clone());

    block_builder.build_block(Vec::new()).await.unwrap();

    // Ensure that the store's `apply_block()` was called
    assert_eq!(*store.num_apply_block_called.read().await, 1);
}

/// Tests that `build_block()` fails when `get_block_inputs()` fails
#[tokio::test]
async fn test_build_block_failure() {
    let store = Arc::new(MockStoreFailure::default());

    let block_builder = DefaultBlockBuilder::new(store.clone());

    let result = block_builder.build_block(Vec::new()).await;

    // Ensure that the store's `apply_block()` was called
    assert!(matches!(result, Err(BuildBlockError::GetBlockInputsFailed(_))));
}
