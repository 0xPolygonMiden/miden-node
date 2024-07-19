use std::iter;

use miden_objects::{
    block::{Block, BlockAccountUpdate, BlockNoteIndex, BlockNoteTree, NoteBatch},
    crypto::merkle::{Mmr, SimpleSmt},
    notes::Nullifier,
    transaction::OutputNote,
    BlockHeader, Digest, ACCOUNT_TREE_DEPTH,
};

use super::MockStoreSuccess;
use crate::{
    block::BlockInputs,
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
    let updated_accounts: Vec<_> =
        batches.iter().flat_map(TransactionBatch::updated_accounts).collect();
    let new_account_root = {
        let mut store_accounts = store.accounts.read().await.clone();
        for update in updated_accounts {
            store_accounts.insert(update.account_id().into(), update.new_state_hash().into());
        }

        store_accounts.root()
    };

    // Compute new chain MMR root
    let new_chain_mmr_root = {
        let mut store_chain_mmr = store.chain_mmr.read().await.clone();

        store_chain_mmr.add(last_block_header.hash());

        store_chain_mmr.peaks(store_chain_mmr.forest()).unwrap().hash_peaks()
    };

    let note_created_smt = note_created_smt_from_note_batches(block_output_notes(batches.iter()));

    // Build header
    BlockHeader::new(
        0,
        last_block_header.hash(),
        last_block_header.block_num() + 1,
        new_chain_mmr_root,
        new_account_root,
        // FIXME: FILL IN CORRECT NULLIFIER ROOT
        Digest::default(),
        note_created_smt.root(),
        Digest::default(),
        Digest::default(),
        1,
    )
}

/// Builds the "actual" block header; i.e. the block header built using the Miden VM, used in the
/// node
pub async fn build_actual_block_header(
    store: &MockStoreSuccess,
    batches: Vec<TransactionBatch>,
) -> BlockHeader {
    let updated_accounts: Vec<_> =
        batches.iter().flat_map(TransactionBatch::updated_accounts).collect();
    let produced_nullifiers: Vec<Nullifier> =
        batches.iter().flat_map(TransactionBatch::produced_nullifiers).collect();

    let block_inputs_from_store: BlockInputs = store
        .get_block_inputs(
            updated_accounts.iter().map(|update| update.account_id()),
            produced_nullifiers.iter(),
            iter::empty(),
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

    updated_accounts: Option<Vec<BlockAccountUpdate>>,
    created_notes: Option<Vec<NoteBatch>>,
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

    pub fn account_updates(mut self, updated_accounts: Vec<BlockAccountUpdate>) -> Self {
        for update in &updated_accounts {
            self.store_accounts
                .insert(update.account_id().into(), update.new_state_hash().into());
        }

        self.updated_accounts = Some(updated_accounts);

        self
    }

    pub fn created_notes(mut self, created_notes: Vec<NoteBatch>) -> Self {
        self.created_notes = Some(created_notes);

        self
    }

    pub fn produced_nullifiers(mut self, produced_nullifiers: Vec<Nullifier>) -> Self {
        self.produced_nullifiers = Some(produced_nullifiers);

        self
    }

    pub fn build(self) -> Block {
        let created_notes = self.created_notes.unwrap_or_default();

        let header = BlockHeader::new(
            0,
            self.last_block_header.hash(),
            self.last_block_header.block_num() + 1,
            self.store_chain_mmr.peaks(self.store_chain_mmr.forest()).unwrap().hash_peaks(),
            self.store_accounts.root(),
            Digest::default(),
            note_created_smt_from_note_batches(created_notes.iter()).root(),
            Digest::default(),
            Digest::default(),
            1,
        );

        Block::new(
            header,
            self.updated_accounts.unwrap_or_default(),
            created_notes,
            self.produced_nullifiers.unwrap_or_default(),
        )
        .unwrap()
    }
}

pub(crate) fn flatten_output_notes<'a>(
    batches: impl Iterator<Item = &'a NoteBatch>,
) -> impl Iterator<Item = (BlockNoteIndex, &'a OutputNote)> {
    batches.enumerate().flat_map(|(batch_idx, batch)| {
        batch.iter().enumerate().map(move |(note_idx_in_batch, note)| {
            (BlockNoteIndex::new(batch_idx, note_idx_in_batch), note)
        })
    })
}

pub(crate) fn note_created_smt_from_note_batches<'a>(
    batches: impl Iterator<Item = &'a NoteBatch>,
) -> BlockNoteTree {
    let note_leaf_iterator = flatten_output_notes(batches)
        .map(|(index, note)| (index, note.id().into(), *note.metadata()));

    BlockNoteTree::with_entries(note_leaf_iterator).unwrap()
}

pub(crate) fn block_output_notes<'a>(
    batches: impl Iterator<Item = &'a TransactionBatch> + Clone,
) -> impl Iterator<Item = &'a NoteBatch> + Clone {
    batches.map(TransactionBatch::output_notes)
}
