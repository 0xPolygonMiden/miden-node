use super::*;

use miden_objects::{transaction::ConsumedNoteInfo, Hasher};

use crate::test_utils::{DummyProvenTxGenerator, MockPrivateAccount};

mod apply_block;
mod verify_tx;

// HELPERS
// -------------------------------------------------------------------------------------------------

pub fn consumed_note_by_index(index: u32) -> ConsumedNoteInfo {
    ConsumedNoteInfo::new(
        Hasher::hash(&index.to_be_bytes()),
        Hasher::hash(
            &[index.to_be_bytes(), index.to_be_bytes()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
        ),
    )
}

/// Returns `num` transactions, and the corresponding account they modify.
/// The transactions each consume a single different note
pub fn get_txs_and_accounts(
    tx_gen: &DummyProvenTxGenerator,
    num: u32,
) -> impl Iterator<Item = (SharedProvenTx, MockPrivateAccount)> + '_ {
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
