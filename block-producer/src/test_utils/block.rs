use std::collections::BTreeMap;

use miden_objects::{
    accounts::AccountId,
    crypto::merkle::{Mmr, SimpleSmt},
    notes::{NoteEnvelope, Nullifier},
    BlockHeader, Digest, ACCOUNT_TREE_DEPTH, BLOCK_OUTPUT_NOTES_TREE_DEPTH, MAX_NOTES_PER_BATCH,
    ONE, ZERO,
};

use super::MockStoreSuccess;
use crate::{
    block::{Block, BlockInputs},
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
        batches.iter().flat_map(TransactionBatch::updated_accounts).collect();
    let new_account_root = {
        let mut store_accounts = store.accounts.read().await.clone();
        for (account_id, new_account_state) in updated_accounts {
            store_accounts.insert(account_id.into(), new_account_state.into());
        }

        store_accounts.root()
    };

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
        note_created_smt_from_batches(batches.iter()).root(),
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
    let produced_nullifiers: Vec<Nullifier> =
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
    produced_nullifiers: Option<Vec<Nullifier>>,
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
        produced_nullifiers: Vec<Nullifier>,
    ) -> Self {
        self.produced_nullifiers = Some(produced_nullifiers);

        self
    }

    pub fn build(self) -> Block {
        let created_notes = self.created_notes.unwrap_or_default();

        let header = BlockHeader::new(
            self.last_block_header.hash(),
            self.last_block_header.block_num() + 1,
            self.store_chain_mmr.peaks(self.store_chain_mmr.forest()).unwrap().hash_peaks(),
            self.store_accounts.root(),
            Digest::default(),
            note_created_smt_from_envelopes(created_notes.iter()).root(),
            Digest::default(),
            Digest::default(),
            ZERO,
            ONE,
        );

        Block {
            header,
            updated_accounts: self.updated_accounts.unwrap_or_default(),
            created_notes,
            produced_nullifiers: self.produced_nullifiers.unwrap_or_default(),
        }
    }
}

pub(crate) fn note_created_smt_from_envelopes<'a>(
    note_iterator: impl Iterator<Item = (&'a u64, &'a NoteEnvelope)>
) -> SimpleSmt<BLOCK_OUTPUT_NOTES_TREE_DEPTH> {
    SimpleSmt::<BLOCK_OUTPUT_NOTES_TREE_DEPTH>::with_leaves(note_iterator.flat_map(
        |(note_idx_in_block, note)| {
            let index = note_idx_in_block * 2;
            [(index, note.note_id().into()), (index + 1, note.metadata().into())]
        },
    ))
    .unwrap()
}

pub(crate) fn note_created_smt_from_batches<'a>(
    batches: impl Iterator<Item = &'a TransactionBatch>
) -> SimpleSmt<BLOCK_OUTPUT_NOTES_TREE_DEPTH> {
    let note_leaf_iterator = batches.enumerate().flat_map(|(batch_index, batch)| {
        let subtree_index = batch_index * MAX_NOTES_PER_BATCH * 2;
        batch.created_notes().enumerate().flat_map(move |(note_index, note)| {
            let index = (subtree_index + note_index * 2) as u64;
            [(index, note.note_id().into()), (index + 1, note.metadata().into())]
        })
    });

    SimpleSmt::<BLOCK_OUTPUT_NOTES_TREE_DEPTH>::with_leaves(note_leaf_iterator).unwrap()
}
