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


/// Tests the happy path where 3 transactions who modify different accounts and consume different
/// notes all verify successfully
#[test]
fn test_verify_tx_happy_path() {

}
