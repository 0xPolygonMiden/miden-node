use std::collections::BTreeSet;

// block builder tests (higher level)
// 1. `apply_block()` is called
// 2. if `apply_block()` fails, you fail too
use super::*;

use miden_air::Felt;

use crate::test_utils::MockStoreSuccess;

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
