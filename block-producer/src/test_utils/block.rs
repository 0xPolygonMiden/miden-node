use miden_air::{Felt, FieldElement};
use miden_objects::{accounts::AccountId, crypto::merkle::Mmr, BlockHeader, Digest};
use miden_vm::crypto::SimpleSmt;

use crate::block::Block;

use super::MockStoreSuccess;

#[derive(Debug)]
pub struct MockBlockBuilder {
    store_accounts: SimpleSmt,
    store_chain_mmr: Mmr,
    last_block_header: BlockHeader,

    updated_accounts: Option<Vec<(AccountId, Digest)>>,
    created_notes: Option<Vec<Digest>>,
    produced_nullifiers: Option<Vec<Digest>>,
}

impl MockBlockBuilder {
    pub async fn new(store: &MockStoreSuccess) -> Self {
        Self {
            store_accounts: store.accounts.read().await.clone(),
            store_chain_mmr: store.chain_mmr.read().await.clone(),
            last_block_header: *store.last_block_header.read().await,

            updated_accounts: None,
            created_notes: None,
            produced_nullifiers: None,
        }
    }

    pub fn account_updates(
        mut self,
        updated_accounts: Vec<(AccountId, Digest)>,
    ) -> Self {
        for (account_id, new_account_state) in updated_accounts.iter() {
            self.store_accounts
                .update_leaf(u64::from(*account_id), new_account_state.into())
                .unwrap();
        }

        self.updated_accounts = Some(updated_accounts);

        self
    }

    pub fn created_notes(
        mut self,
        created_notes: Vec<Digest>,
    ) -> Self {
        self.created_notes = Some(created_notes);

        self
    }

    pub fn produced_nullifiers(
        mut self,
        produced_nullifiers: Vec<Digest>,
    ) -> Self {
        self.produced_nullifiers = Some(produced_nullifiers);

        self
    }

    pub fn build(self) -> Block {
        let header = BlockHeader::new(
            self.last_block_header.hash(),
            self.last_block_header.block_num() + Felt::ONE,
            self.store_chain_mmr.peaks(self.store_chain_mmr.forest()).unwrap().hash_peaks(),
            self.store_accounts.root(),
            Digest::default(),
            Digest::default(),
            Digest::default(),
            Digest::default(),
            Felt::ZERO,
            Felt::ONE,
        );

        Block {
            header,
            updated_accounts: self.updated_accounts.unwrap_or_default(),
            created_notes: self.created_notes.unwrap_or_default(),
            produced_nullifiers: self.produced_nullifiers.unwrap_or_default(),
        }
    }
}
