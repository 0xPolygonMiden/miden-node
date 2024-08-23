//! `verify_tx(tx)` requirements:
//!
//! Store-related requirements
//! VT1: `tx.initial_account_hash` must match the account hash in store
//! VT2: If store doesn't contain account, `verify_tx` should check that it is a new account ( TODO
//! ) and succeed VT3: If `tx` consumes an already-consumed note in the store, `verify_tx` must fail
//!
//! in-flight related requirements
//! VT4: In each block, at most 1 transaction is allowed to modify any given account
//! VT5: `verify_tx(tx)` must fail if a previous transaction, not yet in the block, consumed a note
//!      that `tx` is also consuming

use std::iter;

use miden_objects::notes::Note;
use tokio::task::JoinSet;

use super::*;
use crate::test_utils::{block::MockBlockBuilder, note::mock_note, MockStoreSuccessBuilder};

/// Tests the happy path where 3 transactions who modify different accounts and consume different
/// notes all verify successfully
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_verify_tx_happy_path() {
    let (txs, accounts): (Vec<ProvenTransaction>, Vec<MockPrivateAccount>) =
        get_txs_and_accounts(0, 3).unzip();

    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(
            accounts
                .into_iter()
                .map(|mock_account| (mock_account.id, mock_account.states[0])),
        )
        .build(),
    );

    let state_view = DefaultStateView::new(store, false);

    for tx in txs {
        state_view.verify_tx(&tx).await.unwrap();
    }
}

/// Tests the happy path where 3 transactions who modify different accounts and consume different
/// notes all verify successfully.
///
/// In this test, all calls to `verify_tx()` are concurrent
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_verify_tx_happy_path_concurrent() {
    let (txs, accounts): (Vec<ProvenTransaction>, Vec<MockPrivateAccount>) =
        get_txs_and_accounts(0, 3).unzip();

    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(
            accounts
                .into_iter()
                .map(|mock_account| (mock_account.id, mock_account.states[0])),
        )
        .build(),
    );

    let state_view = Arc::new(DefaultStateView::new(store, false));

    let mut set = JoinSet::new();

    for tx in txs {
        let state_view = state_view.clone();
        set.spawn(async move { state_view.verify_tx(&tx).await });
    }

    while let Some(res) = set.join_next().await {
        res.unwrap().unwrap();
    }
}

/// Verifies requirement VT1
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_verify_tx_vt1() {
    let account = MockPrivateAccount::<3>::from(1);

    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(iter::once((account.id, account.states[0]))).build(),
    );

    // The transaction's initial account hash uses `account.states[1]`, where the store expects
    // `account.states[0]`
    let tx = MockProvenTxBuilder::with_account(account.id, account.states[1], account.states[2])
        .nullifiers_range(0..1)
        .build();

    let state_view = DefaultStateView::new(store, false);

    let verify_tx_result = state_view.verify_tx(&tx).await;

    assert_eq!(
        verify_tx_result,
        Err(VerifyTxError::IncorrectAccountInitialHash {
            tx_initial_account_hash: account.states[1],
            current_account_hash: Some(account.states[0]),
        })
    );
}

/// Verifies requirement VT2
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_verify_tx_vt2() {
    let account_not_in_store: MockPrivateAccount<3> = MockPrivateAccount::from(0);

    // Notice: account is not added to the store
    let store = Arc::new(MockStoreSuccessBuilder::from_batches(iter::empty()).build());

    let tx = MockProvenTxBuilder::with_account(
        account_not_in_store.id,
        account_not_in_store.states[0],
        account_not_in_store.states[1],
    )
    .nullifiers_range(0..1)
    .build();

    let state_view = DefaultStateView::new(store, false);

    let verify_tx_result = state_view.verify_tx(&tx).await;

    assert!(verify_tx_result.is_ok());
}

/// Verifies requirement VT3
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_verify_tx_vt3() {
    let account: MockPrivateAccount<3> = MockPrivateAccount::from(1);

    let nullifier_in_store = nullifier_by_index(0);

    // Notice: `consumed_note_in_store` is added to the store
    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(iter::once((account.id, account.states[0])))
            .initial_nullifiers(BTreeSet::from_iter(iter::once(nullifier_in_store.inner())))
            .initial_block_num(1)
            .build(),
    );

    let tx = MockProvenTxBuilder::with_account(account.id, account.states[0], account.states[1])
        .nullifiers(vec![nullifier_in_store])
        .build();

    let state_view = DefaultStateView::new(store, false);

    let verify_tx_result = state_view.verify_tx(&tx).await;

    assert_eq!(
        verify_tx_result,
        Err(VerifyTxError::InputNotesAlreadyConsumed(vec![nullifier_in_store]))
    );
}

/// Verifies requirement VT4
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_verify_tx_vt4() {
    let account: MockPrivateAccount<3> = MockPrivateAccount::from(1);

    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(iter::once((account.id, account.states[0]))).build(),
    );

    let tx1 =
        MockProvenTxBuilder::with_account(account.id, account.states[0], account.states[1]).build();

    // Notice: tx2 follows tx1, using the same account and with an initial state matching the final
    // state of the first.         We expect both to pass.
    let tx2 =
        MockProvenTxBuilder::with_account(account.id, account.states[1], account.states[2]).build();

    let state_view = DefaultStateView::new(store, false);

    let verify_tx1_result = state_view.verify_tx(&tx1).await;
    assert!(verify_tx1_result.is_ok());

    let verify_tx2_result = state_view.verify_tx(&tx2).await;
    assert!(verify_tx2_result.is_ok());
}

/// Verifies requirement VT5
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_verify_tx_vt5() {
    let account_1: MockPrivateAccount<3> = MockPrivateAccount::from(1);
    let account_2: MockPrivateAccount<3> = MockPrivateAccount::from(2);
    let nullifier_in_both_txs = nullifier_by_index(0);

    // Notice: `consumed_note_in_both_txs` is NOT in the store
    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(
            [account_1, account_2]
                .into_iter()
                .map(|account| (account.id, account.states[0])),
        )
        .build(),
    );

    let tx1 =
        MockProvenTxBuilder::with_account(account_1.id, account_1.states[0], account_1.states[1])
            .nullifiers(vec![nullifier_in_both_txs])
            .build();

    // Notice: tx2 modifies the same account as tx1, even though from a different initial state,
    // which is currently disallowed
    let tx2 =
        MockProvenTxBuilder::with_account(account_2.id, account_2.states[1], account_2.states[2])
            .nullifiers(vec![nullifier_in_both_txs])
            .build();

    let state_view = DefaultStateView::new(store, false);

    let verify_tx1_result = state_view.verify_tx(&tx1).await;
    assert!(verify_tx1_result.is_ok());

    let verify_tx2_result = state_view.verify_tx(&tx2).await;
    assert_eq!(
        verify_tx2_result,
        Err(VerifyTxError::InputNotesAlreadyConsumed(vec![nullifier_in_both_txs]))
    );
}

/// Tests that `verify_tx()` succeeds when the unauthenticated input note found in the in-flight
/// notes
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_verify_tx_dangling_note_found_in_inflight_notes() {
    let account_1: MockPrivateAccount<3> = MockPrivateAccount::from(1);
    let account_2: MockPrivateAccount<3> = MockPrivateAccount::from(2);
    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(
            [account_1, account_2]
                .into_iter()
                .map(|account| (account.id, account.states[0])),
        )
        .build(),
    );
    let state_view = DefaultStateView::new(Arc::clone(&store), false);

    let dangling_notes = vec![mock_note(1)];
    let output_notes = dangling_notes.iter().cloned().map(OutputNote::Full).collect();

    let tx1 = MockProvenTxBuilder::with_account_index(1).output_notes(output_notes).build();

    let verify_tx1_result = state_view.verify_tx(&tx1).await;
    assert_eq!(verify_tx1_result, Ok(Some(0)));

    let tx2 = MockProvenTxBuilder::with_account_index(2)
        .unauthenticated_notes(dangling_notes.clone())
        .build();

    let verify_tx2_result = state_view.verify_tx(&tx2).await;
    assert_eq!(
        verify_tx2_result,
        Ok(Some(0)),
        "Dangling unauthenticated notes must be found in the in-flight notes after previous tx verification"
    );
}

/// Tests that `verify_tx()` fails when the unauthenticated input note not found not in the
/// in-flight notes nor in the store
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn test_verify_tx_stored_unauthenticated_notes() {
    let account_1: MockPrivateAccount<3> = MockPrivateAccount::from(1);
    let store = Arc::new(
        MockStoreSuccessBuilder::from_accounts(
            [account_1].into_iter().map(|account| (account.id, account.states[0])),
        )
        .build(),
    );
    let dangling_notes = vec![mock_note(1)];
    let tx1 = MockProvenTxBuilder::with_account_index(1)
        .unauthenticated_notes(dangling_notes.clone())
        .build();

    let state_view = DefaultStateView::new(Arc::clone(&store), false);

    let verify_tx1_result = state_view.verify_tx(&tx1).await;
    assert_eq!(
        verify_tx1_result,
        Err(VerifyTxError::UnauthenticatedNotesNotFound(
            dangling_notes.iter().map(Note::id).collect()
        )),
        "Dangling unauthenticated notes must not be found in the store by this moment"
    );

    let output_notes = dangling_notes.into_iter().map(OutputNote::Full).collect();
    let block = MockBlockBuilder::new(&store).await.created_notes(vec![output_notes]).build();

    store.apply_block(&block).await.unwrap();

    let verify_tx1_result = state_view.verify_tx(&tx1).await;
    assert_eq!(
        verify_tx1_result,
        Ok(Some(0)),
        "Dangling unauthenticated notes must be found in the store after block applying"
    );
}
