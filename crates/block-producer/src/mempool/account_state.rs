use std::collections::{BTreeMap, BTreeSet};

use miden_objects::{accounts::AccountId, transaction::TransactionId, Digest};

#[derive(Default)]
pub struct AccountState {
    accounts: BTreeMap<AccountId, (Digest, TransactionId)>,
}

impl AccountState {
    pub fn get(&self, account: &AccountId) -> Option<&(Digest, TransactionId)> {
        self.accounts.get(account)
    }

    pub fn insert(&mut self, account: AccountId, state: Digest, transaction: TransactionId) {
        self.accounts.insert(account, (state, transaction));
    }

    pub fn revert_transactions(&mut self, transactions: BTreeSet<TransactionId>) {
        // Implementing this will require some sort of linked list of account updates.
        // Otherwise the original state/transaction ID is lost.
        unimplemented!();
    }
}
