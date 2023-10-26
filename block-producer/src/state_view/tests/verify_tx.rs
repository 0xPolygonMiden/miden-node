//! `verify_tx(tx)` requirements:
//!
//! Store-related requirements
//! VT1: `tx.initial_account_hash` must match the account hash in store
//! VT2: If store doesn't contain account, `verify_tx` must fail
//! VT3: If `tx` consumes an already-consumed note in the store, `verify_tx` must fail
//!
//! in-flight related requirements
//! VT4: In each block, at most 1 transaction is allowed to modify any given account
//! VT5: `verify_tx(tx)` must fail if a previous transaction, not yet in the block, consumed a note
//!      that `tx` is also consuming

use std::iter;

use tokio::task::JoinSet;

use super::*;

/// Tests the happy path where 3 transactions who modify different accounts and consume different
/// notes all verify successfully
#[tokio::test]
async fn test_verify_tx_happy_path() {
    let tx_gen = DummyProvenTxGenerator::new();
    let (txs, accounts): (Vec<ProvenTransaction>, Vec<MockPrivateAccount>) =
        get_txs_and_accounts(&tx_gen, 3).unzip();

    let store = Arc::new(MockStoreSuccess::new(accounts.into_iter(), BTreeSet::new()));

    let state_view = DefaulStateView::new(store);

    for tx in txs {
        state_view.verify_tx(Arc::new(tx)).await.unwrap();
    }
}

/// Tests the happy path where 3 transactions who modify different accounts and consume different
/// notes all verify successfully.
///
/// In this test, all calls to `verify_tx()` are concurrent
#[tokio::test]
async fn test_verify_tx_happy_path_concurrent() {
    let tx_gen = DummyProvenTxGenerator::new();
    let (txs, accounts): (Vec<ProvenTransaction>, Vec<MockPrivateAccount>) =
        get_txs_and_accounts(&tx_gen, 3).unzip();

    let store = Arc::new(MockStoreSuccess::new(accounts.into_iter(), BTreeSet::new()));

    let state_view = Arc::new(DefaulStateView::new(store));

    let mut set = JoinSet::new();

    for tx in txs {
        let state_view = state_view.clone();
        let tx = Arc::new(tx);
        set.spawn(async move { state_view.verify_tx(tx).await });
    }

    while let Some(res) = set.join_next().await {
        res.unwrap().unwrap();
    }
}

/// Verifies requirement VT1
#[tokio::test]
async fn test_verify_tx_vt1() {
    let tx_gen = DummyProvenTxGenerator::new();

    let account = MockPrivateAccount::from(0);

    let store = Arc::new(MockStoreSuccess::new(iter::once(account), BTreeSet::new()));

    // The transaction's initial account hash uses `account.states[1]`, where the store expects
    // `account.states[0]`
    let tx = tx_gen.dummy_proven_tx_with_params(
        account.id,
        account.states[1],
        account.states[2],
        vec![consumed_note_by_index(0)],
    );

    let state_view = DefaulStateView::new(store);

    let verify_tx_result = state_view.verify_tx(tx.into()).await;

    assert_eq!(
        verify_tx_result,
        Err(VerifyTxError::IncorrectAccountInitialHash {
            tx_initial_account_hash: account.states[1],
            store_account_hash: Some(account.states[0])
        })
    );
}

/// Verifies requirement VT2
#[tokio::test]
async fn test_verify_tx_vt2() {
    let tx_gen = DummyProvenTxGenerator::new();

    let account_not_in_store: MockPrivateAccount<3> = MockPrivateAccount::from(0);

    // Notice: account is not added to the store
    let store = Arc::new(MockStoreSuccess::new(iter::empty(), BTreeSet::new()));

    let tx = tx_gen.dummy_proven_tx_with_params(
        account_not_in_store.id,
        account_not_in_store.states[0],
        account_not_in_store.states[1],
        vec![consumed_note_by_index(0)],
    );

    let state_view = DefaulStateView::new(store);

    let verify_tx_result = state_view.verify_tx(tx.into()).await;

    assert_eq!(
        verify_tx_result,
        Err(VerifyTxError::IncorrectAccountInitialHash {
            tx_initial_account_hash: account_not_in_store.states[0],
            store_account_hash: None
        })
    );
}

/// Verifies requirement VT3
#[tokio::test]
async fn test_verify_tx_vt3() {
    let tx_gen = DummyProvenTxGenerator::new();

    let account: MockPrivateAccount<3> = MockPrivateAccount::from(0);

    let consumed_note_in_store = consumed_note_by_index(0);

    // Notice: `consumed_note_in_store` is added to the store
    let store = Arc::new(MockStoreSuccess::new(
        iter::once(account),
        BTreeSet::from_iter(iter::once(consumed_note_in_store.nullifier())),
    ));

    let tx = tx_gen.dummy_proven_tx_with_params(
        account.id,
        account.states[0],
        account.states[1],
        vec![consumed_note_in_store],
    );

    let state_view = DefaulStateView::new(store);

    let verify_tx_result = state_view.verify_tx(tx.into()).await;

    assert_eq!(
        verify_tx_result,
        Err(VerifyTxError::ConsumedNotesAlreadyConsumed(vec![
            consumed_note_in_store.nullifier()
        ]))
    );
}

/// Verifies requirement VT4
#[tokio::test]
async fn test_verify_tx_vt4() {
    let tx_gen = DummyProvenTxGenerator::new();

    let account: MockPrivateAccount<3> = MockPrivateAccount::from(0);

    let store = Arc::new(MockStoreSuccess::new(iter::once(account), BTreeSet::new()));

    let tx1 = tx_gen.dummy_proven_tx_with_params(
        account.id,
        account.states[0],
        account.states[1],
        Vec::new(),
    );

    // Notice: tx2 modifies the same account as tx1, even though from a different initial state,
    // which is currently disallowed
    let tx2 = tx_gen.dummy_proven_tx_with_params(
        account.id,
        account.states[1],
        account.states[2],
        Vec::new(),
    );

    let state_view = DefaulStateView::new(store);

    let verify_tx1_result = state_view.verify_tx(tx1.into()).await;
    assert!(verify_tx1_result.is_ok());

    let verify_tx2_result = state_view.verify_tx(tx2.into()).await;
    assert_eq!(
        verify_tx2_result,
        Err(VerifyTxError::AccountAlreadyModifiedByOtherTx(account.id))
    );
}
