//! Requirements for `apply_block()`:
//!
//! AB1: the internal store's `apply_block` is called once
//! AB2: All accounts modified by transactions in the block are removed from the internal state
//! AB3: All consumed notes by some transaction in the block are still not consumable after `apply_block`

use std::iter;

use super::*;
use crate::test_utils::{block::MockBlockBuilder, MockStoreSuccessBuilder};

/// Tests requirement AB1
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_apply_block_ab1() {
    let account: MockPrivateAccount<3> = MockPrivateAccount::from(0);

    let store = Arc::new(
        MockStoreSuccessBuilder::new()
            .initial_accounts(iter::once((account.id, account.states[0])))
            .build(),
    );

    let tx =
        MockProvenTxBuilder::with_account(account.id, account.states[0], account.states[1]).build();

    let state_view = DefaultStateView::new(store.clone(), false);

    // Verify transaction so it can be tracked in state view
    let verify_tx_res = state_view.verify_tx(&tx).await;
    assert!(verify_tx_res.is_ok());

    let block = MockBlockBuilder::new(&store)
        .await
        .account_updates(
            std::iter::once(account)
                .map(|mock_account| (mock_account.id, mock_account.states[1]))
                .collect(),
        )
        .build();

    let apply_block_res = state_view.apply_block(block).await;
    assert!(apply_block_res.is_ok());

    assert_eq!(*store.num_apply_block_called.read().await, 1);
}

/// Tests requirement AB2
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_apply_block_ab2() {
    let (txs, accounts): (Vec<_>, Vec<_>) = get_txs_and_accounts(3).unzip();

    let store = Arc::new(
        MockStoreSuccessBuilder::new()
            .initial_accounts(
                accounts
                    .clone()
                    .into_iter()
                    .map(|mock_account| (mock_account.id, mock_account.states[0])),
            )
            .build(),
    );

    let state_view = DefaultStateView::new(store.clone(), false);

    // Verify transactions so it can be tracked in state view
    for tx in txs {
        let verify_tx_res = state_view.verify_tx(&tx).await;
        assert!(verify_tx_res.is_ok());
    }

    // All except the first account will go into the block.
    let accounts_in_block: Vec<MockPrivateAccount> = accounts.iter().skip(1).cloned().collect();

    let block = MockBlockBuilder::new(&store)
        .await
        .account_updates(
            accounts_in_block
                .into_iter()
                .map(|mock_account| (mock_account.id, mock_account.states[1]))
                .collect(),
        )
        .build();

    let apply_block_res = state_view.apply_block(block).await;
    assert!(apply_block_res.is_ok());

    let accounts_still_in_flight = state_view.accounts_in_flight.read().await;

    // Only the first account should still be in flight
    assert_eq!(accounts_still_in_flight.len(), 1);
    assert!(accounts_still_in_flight.contains(&accounts[0].id));
}

/// Tests requirement AB3
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_apply_block_ab3() {
    let (txs, accounts): (Vec<_>, Vec<_>) = get_txs_and_accounts(3).unzip();

    let store = Arc::new(
        MockStoreSuccessBuilder::new()
            .initial_accounts(
                accounts
                    .clone()
                    .into_iter()
                    .map(|mock_account| (mock_account.id, mock_account.states[0])),
            )
            .build(),
    );

    let state_view = DefaultStateView::new(store.clone(), false);

    // Verify transactions so it can be tracked in state view
    for tx in txs.clone() {
        let verify_tx_res = state_view.verify_tx(&tx).await;
        assert!(verify_tx_res.is_ok());
    }

    let block = MockBlockBuilder::new(&store)
        .await
        .account_updates(
            accounts
                .clone()
                .into_iter()
                .map(|mock_account| (mock_account.id, mock_account.states[1]))
                .collect(),
        )
        .build();

    let apply_block_res = state_view.apply_block(block).await;
    assert!(apply_block_res.is_ok());

    // Craft a new transaction which tries to consume the same note that was consumed in in the
    // first tx
    let tx_new = MockProvenTxBuilder::with_account(
        accounts[0].id,
        accounts[0].states[1],
        accounts[0].states[2],
    )
    .nullifiers(txs[0].input_notes().iter().cloned().collect())
    .build();

    let verify_tx_res = state_view.verify_tx(&tx_new).await;
    assert_eq!(
        verify_tx_res,
        Err(VerifyTxError::InputNotesAlreadyConsumed(txs[0].input_notes().clone()))
    );
}
