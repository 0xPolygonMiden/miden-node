use miden_objects::{Hasher, EMPTY_WORD, ZERO};

use super::*;
use crate::test_utils::{MockPrivateAccount, MockProvenTxBuilder};

mod apply_block;
mod verify_tx;

// HELPERS
// -------------------------------------------------------------------------------------------------

pub fn nullifier_by_index(index: u32) -> Nullifier {
    Nullifier::new(
        Hasher::hash(&index.to_be_bytes()),
        Hasher::hash(
            &[index.to_be_bytes(), index.to_be_bytes()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
        ),
        EMPTY_WORD.into(),
        [ZERO, ZERO, ZERO, index.into()],
    )
}

/// Returns `num` transactions, and the corresponding account they modify.
/// The transactions each consume a single different note
pub fn get_txs_and_accounts(
    starting_account_index: u32,
    num: u32,
) -> impl Iterator<Item=(ProvenTransaction, MockPrivateAccount)> {
    (0..num).map(move |index| {
        let account = MockPrivateAccount::from(starting_account_index + index);
        let nullifier_starting_index = (starting_account_index + index) as u64;
        let tx =
            MockProvenTxBuilder::with_account(account.id, account.states[0], account.states[1])
                .nullifiers_range(nullifier_starting_index..(nullifier_starting_index + 1))
                .build();

        (tx, account)
    })
}
