use std::collections::BTreeMap;

use miden_node_proto::domain::blocks::BlockInputs;
use miden_objects::{
    accounts::AccountId, crypto::merkle::Mmr, notes::NoteEnvelope, BlockHeader, Digest,
    ACCOUNT_TREE_DEPTH, ONE, ZERO,
};
use miden_vm::crypto::SimpleSmt;

use super::MockStoreSuccess;
use crate::{
    block::Block,
    block_builder::prover::{block_witness::BlockWitness, BlockProver},
    store::Store,
    TransactionBatch,
};

/// Constructs the block we expect to be built given the store state, and a set of transaction
/// batches to be applied
pub async fn build_expected_block_header(
    store: &MockStoreSuccess,
    batches: &[TransactionBatch],
) -> BlockHeader {
    let last_block_header = *store.last_block_header.read().await;

    // Compute new account root
    let updated_accounts: Vec<(AccountId, Digest)> =
        batches.iter().flat_map(|batch| batch.updated_accounts()).collect();
    let new_account_root = {
        let mut store_accounts = store.accounts.read().await.clone();
        for &(account_id, new_account_state) in updated_accounts.iter() {
            store_accounts.insert(account_id.into(), new_account_state.into());
        }

        store_accounts.root()
    };

    // Compute created notes root
    // FIXME: compute the right root. Needs
    // https://github.com/0xPolygonMiden/crypto/issues/220#issuecomment-1823911017
    let new_created_notes_root = Digest::default();

    // Compute new chain MMR root
    let new_chain_mmr_root = {
        let mut store_chain_mmr = store.chain_mmr.read().await.clone();

        store_chain_mmr.add(last_block_header.hash());

        store_chain_mmr.peaks(store_chain_mmr.forest()).unwrap().hash_peaks()
    };

    // Build header
    BlockHeader::new(
        last_block_header.hash(),
        last_block_header.block_num() + 1,
        new_chain_mmr_root,
        new_account_root,
        // FIXME: FILL IN CORRECT NULLIFIER ROOT
        Digest::default(),
        // FIXME: FILL IN CORRECT CREATED NOTES ROOT
        new_created_notes_root,
        Digest::default(),
        Digest::default(),
        ZERO,
        ONE,
    )
}

/// Builds the "actual" block header; i.e. the block header built using the Miden VM, used in the
/// node
pub async fn build_actual_block_header(
    store: &MockStoreSuccess,
    batches: Vec<TransactionBatch>,
) -> BlockHeader {
    let updated_accounts: Vec<(AccountId, Digest)> =
        batches.iter().flat_map(|batch| batch.updated_accounts()).collect();
    let produced_nullifiers: Vec<Digest> =
        batches.iter().flat_map(|batch| batch.produced_nullifiers()).collect();

    let block_inputs_from_store: BlockInputs = store
        .get_block_inputs(
            updated_accounts.iter().map(|(account_id, _)| account_id),
            produced_nullifiers.iter(),
        )
        .await
        .unwrap();

    let block_witness = BlockWitness::new(block_inputs_from_store, &batches).unwrap();

    BlockProver::new().prove(block_witness).unwrap()
}

#[derive(Debug)]
pub struct MockBlockBuilder {
    store_accounts: SimpleSmt<ACCOUNT_TREE_DEPTH>,
    store_chain_mmr: Mmr,
    last_block_header: BlockHeader,

    updated_accounts: Option<Vec<(AccountId, Digest)>>,
    created_notes: Option<BTreeMap<u64, NoteEnvelope>>,
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
        for &(account_id, new_account_state) in updated_accounts.iter() {
            self.store_accounts.insert(account_id.into(), new_account_state.into());
        }

        self.updated_accounts = Some(updated_accounts);

        self
    }

    pub fn created_notes(
        mut self,
        created_notes: BTreeMap<u64, NoteEnvelope>,
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
            self.last_block_header.block_num() + 1,
            self.store_chain_mmr.peaks(self.store_chain_mmr.forest()).unwrap().hash_peaks(),
            self.store_accounts.root(),
            Digest::default(),
            Digest::default(),
            Digest::default(),
            Digest::default(),
            ZERO,
            ONE,
        );

        Block {
            header,
            updated_accounts: self.updated_accounts.unwrap_or_default(),
            created_notes: self.created_notes.unwrap_or_default(),
            produced_nullifiers: self.produced_nullifiers.unwrap_or_default(),
        }
    }
}
