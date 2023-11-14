use super::*;

use miden_objects::{transaction::ConsumedNoteInfo, BlockHeader, Felt, Hasher};

use crate::test_utils::{DummyProvenTxGenerator, MockPrivateAccount};

mod apply_block;
mod verify_tx;

// HELPERS
// -------------------------------------------------------------------------------------------------

pub fn consumed_note_by_index(index: u8) -> ConsumedNoteInfo {
    ConsumedNoteInfo::new(Hasher::hash(&[index]), Hasher::hash(&[index, index]))
}

/// Returns `num` transactions, and the corresponding account they modify.
/// The transactions each consume a single different note
pub fn get_txs_and_accounts<'a>(
    tx_gen: &'a DummyProvenTxGenerator,
    num: u8,
) -> impl Iterator<Item = (SharedProvenTx, MockPrivateAccount)> + 'a {
    (0..num).map(|index| {
        let account = MockPrivateAccount::from(index);
        let tx = tx_gen.dummy_proven_tx_with_params(
            account.id,
            account.states[0],
            account.states[1],
            vec![consumed_note_by_index(index)],
            Vec::new(),
        );

        (Arc::new(tx), account)
    })
}

pub fn get_dummy_block(
    updated_accounts: Vec<MockPrivateAccount>,
    new_nullifiers: Vec<Digest>,
) -> Block {
    let header = BlockHeader::new(
        Digest::default(),
        Felt::new(42),
        Digest::default(),
        Digest::default(),
        Digest::default(),
        Digest::default(),
        Digest::default(),
        Digest::default(),
        Felt::new(0),
        Felt::new(42),
    );

    let updated_accounts = updated_accounts
        .into_iter()
        .map(|mock_account| (mock_account.id, mock_account.states[1]))
        .collect();

    Block {
        header,
        updated_accounts,
        created_notes: Vec::new(),
        produced_nullifiers: new_nullifiers,
    }
}
