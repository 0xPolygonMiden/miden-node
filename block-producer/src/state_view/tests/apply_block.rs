//! Requirements for `apply_block()`:
//!
//! AB1: the internal store's `apply_block` is called once
//! AB2: All accounts modified by transactions in the block are removed from the internal state
//! AB3: All consumed notes by some transaction in the block are still not consumable after `apply_block`

use std::iter;

use super::*;

/// Tests requirement AB1
#[tokio::test]
async fn test_apply_block_ab1() {
    let tx_gen = DummyProvenTxGenerator::new();
    let account: MockPrivateAccount<3> = MockPrivateAccount::from(0);

    let store = Arc::new(MockStoreSuccess::new(iter::once(account), BTreeSet::new()));

    let tx = tx_gen.dummy_proven_tx_with_params(
        account.id,
        account.states[0],
        account.states[1],
        Vec::new(),
    );

    let state_view = DefaulStateView::new(store.clone());

    // Verify transaction so it can be tracked in state view
    let verify_tx_res = state_view.verify_tx(tx.into()).await;
    assert!(verify_tx_res.is_ok());

    let block = Arc::new(get_dummy_block(vec![account], Vec::new()));

    let apply_block_res = state_view.apply_block(block).await;
    assert!(apply_block_res.is_ok());

    assert_eq!(*store.num_apply_block_called.read().await, 1);
}
