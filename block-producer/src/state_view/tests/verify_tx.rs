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

use super::*;
use crate::test_utils::DummyProvenTxGenerator;

/// Tests the happy path where 3 transactions who modify different accounts and consume different
/// notes all verify successfully
#[test]
fn test_verify_tx_happy_path() {
    let tx_gen = DummyProvenTxGenerator::new();

    let account1 = MockAccount::account_by_index(1);
    let tx1 = tx_gen.dummy_proven_tx_with_params(
        account1.id,
        account1.states[0],
        account1.states[1],
        vec![consumed_note_by_index(1)],
    );

    let account2 = MockAccount::account_by_index(2);
    let tx2 = tx_gen.dummy_proven_tx_with_params(
        account2.id,
        account2.states[0],
        account2.states[1],
        vec![consumed_note_by_index(2)],
    );

    let account3 = MockAccount::account_by_index(3);
    let tx3 = tx_gen.dummy_proven_tx_with_params(
        account3.id,
        account3.states[0],
        account3.states[1],
        vec![consumed_note_by_index(3)],
    );


}
