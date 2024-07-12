use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Not,
};

use async_trait::async_trait;
use miden_objects::{
    block::{Block, BlockNoteTree, NoteBatch},
    crypto::merkle::{MerklePath, Mmr, SimpleSmt, Smt, ValuePath},
    notes::{NoteId, Nullifier},
    BlockHeader, ACCOUNT_TREE_DEPTH, EMPTY_WORD, ZERO,
};

use super::*;
use crate::{
    batch_builder::TransactionBatch,
    block::{AccountWitness, BlockInputs},
    errors::NotePathsError,
    store::{
        ApplyBlock, ApplyBlockError, BlockInputsError, Store, TransactionInputs, TxInputsError,
    },
    test_utils::block::{
        block_output_notes, flatten_output_notes, note_created_smt_from_note_batches,
    },
    ProvenTransaction,
};

/// Builds a [`MockStoreSuccess`]
#[derive(Debug)]
pub struct MockStoreSuccessBuilder {
    accounts: Option<SimpleSmt<ACCOUNT_TREE_DEPTH>>,
    notes: Option<BTreeMap<NoteId, MerklePath>>,
    note_root: Option<Digest>,
    produced_nullifiers: Option<BTreeSet<Digest>>,
    chain_mmr: Option<Mmr>,
    block_num: Option<u32>,
}

impl MockStoreSuccessBuilder {
    pub fn from_batches<'a>(
        batches_iter: impl Iterator<Item = &'a TransactionBatch> + Clone,
    ) -> Self {
        let accounts_smt = {
            let accounts = batches_iter
                .clone()
                .flat_map(TransactionBatch::account_initial_states)
                .map(|(account_id, hash)| (account_id.into(), hash.into()));
            SimpleSmt::<ACCOUNT_TREE_DEPTH>::with_leaves(accounts).unwrap()
        };

        let (note_tree, notes) = Self::populate_note_trees(block_output_notes(batches_iter));

        Self {
            accounts: Some(accounts_smt),
            notes: Some(notes),
            note_root: Some(note_tree.root()),
            produced_nullifiers: None,
            chain_mmr: None,
            block_num: None,
        }
    }

    pub fn from_accounts(accounts: impl Iterator<Item = (AccountId, Digest)>) -> Self {
        let accounts_smt = {
            let accounts = accounts.map(|(account_id, hash)| (account_id.into(), hash.into()));

            SimpleSmt::<ACCOUNT_TREE_DEPTH>::with_leaves(accounts).unwrap()
        };

        Self {
            accounts: Some(accounts_smt),
            notes: None,
            note_root: None,
            produced_nullifiers: None,
            chain_mmr: None,
            block_num: None,
        }
    }

    pub fn initial_notes<'a>(mut self, notes: impl Iterator<Item = &'a NoteBatch> + Clone) -> Self {
        let (note_tree, notes) = Self::populate_note_trees(notes);

        self.notes = Some(notes);
        self.note_root = Some(note_tree.root());

        self
    }

    pub fn initial_nullifiers(mut self, nullifiers: BTreeSet<Digest>) -> Self {
        self.produced_nullifiers = Some(nullifiers);

        self
    }

    pub fn initial_chain_mmr(mut self, chain_mmr: Mmr) -> Self {
        self.chain_mmr = Some(chain_mmr);

        self
    }

    pub fn initial_block_num(mut self, block_num: u32) -> Self {
        self.block_num = Some(block_num);

        self
    }

    fn populate_note_trees<'a>(
        batches_iterator: impl Iterator<Item = &'a NoteBatch> + Clone,
    ) -> (BlockNoteTree, BTreeMap<NoteId, MerklePath>) {
        let block_note_tree = note_created_smt_from_note_batches(batches_iterator.clone());
        let note_map = flatten_output_notes(batches_iterator)
            .map(|(index, note)| (note.id(), block_note_tree.get_note_path(index).unwrap()))
            .collect();

        (block_note_tree, note_map)
    }

    pub fn build(self) -> MockStoreSuccess {
        let block_num = self.block_num.unwrap_or(1);
        let accounts_smt = self.accounts.unwrap_or(SimpleSmt::new().unwrap());
        let notes = self.notes.unwrap_or_default();
        let note_root = self.note_root.unwrap_or_default();
        let chain_mmr = self.chain_mmr.unwrap_or_default();
        let nullifiers_smt = self
            .produced_nullifiers
            .map(|nullifiers| {
                Smt::with_entries(
                    nullifiers
                        .into_iter()
                        .map(|nullifier| (nullifier, [block_num.into(), ZERO, ZERO, ZERO])),
                )
                .unwrap()
            })
            .unwrap_or_default();

        let initial_block_header = BlockHeader::new(
            0,
            Digest::default(),
            block_num,
            chain_mmr.peaks(chain_mmr.forest()).unwrap().hash_peaks(),
            accounts_smt.root(),
            nullifiers_smt.root(),
            note_root,
            Digest::default(),
            Digest::default(),
            1,
        );

        MockStoreSuccess {
            accounts: Arc::new(RwLock::new(accounts_smt)),
            produced_nullifiers: Arc::new(RwLock::new(nullifiers_smt)),
            chain_mmr: Arc::new(RwLock::new(chain_mmr)),
            last_block_header: Arc::new(RwLock::new(initial_block_header)),
            num_apply_block_called: Default::default(),
            notes: Arc::new(RwLock::new(notes)),
        }
    }
}

pub struct MockStoreSuccess {
    /// Map account id -> account hash
    pub accounts: Arc<RwLock<SimpleSmt<ACCOUNT_TREE_DEPTH>>>,

    /// Stores the nullifiers of the notes that were consumed
    pub produced_nullifiers: Arc<RwLock<Smt>>,

    /// Stores the chain MMR
    pub chain_mmr: Arc<RwLock<Mmr>>,

    /// Stores the header of the last applied block
    pub last_block_header: Arc<RwLock<BlockHeader>>,

    /// The number of times `apply_block()` was called
    pub num_apply_block_called: Arc<RwLock<u32>>,

    /// Maps note id -> note inclusion proof for all created notes
    pub notes: Arc<RwLock<BTreeMap<NoteId, MerklePath>>>,
}

impl MockStoreSuccess {
    pub async fn account_root(&self) -> Digest {
        let locked_accounts = self.accounts.read().await;

        locked_accounts.root()
    }
}

#[async_trait]
impl ApplyBlock for MockStoreSuccess {
    async fn apply_block(&self, block: &Block) -> Result<(), ApplyBlockError> {
        // Intentionally, we take and hold both locks, to prevent calls to `get_tx_inputs()` from going through while we're updating the store's data structure
        let mut locked_accounts = self.accounts.write().await;
        let mut locked_produced_nullifiers = self.produced_nullifiers.write().await;

        // update accounts
        for update in block.updated_accounts() {
            locked_accounts.insert(update.account_id().into(), update.new_state_hash().into());
        }
        debug_assert_eq!(locked_accounts.root(), block.header().account_root());

        // update nullifiers
        for nullifier in block.created_nullifiers() {
            locked_produced_nullifiers
                .insert(nullifier.inner(), [block.header().block_num().into(), ZERO, ZERO, ZERO]);
        }

        // update chain mmr with new block header hash
        {
            let mut chain_mmr = self.chain_mmr.write().await;

            chain_mmr.add(block.hash());
        }

        // build note tree
        let note_tree = block.build_note_tree();

        // update notes
        let mut locked_notes = self.notes.write().await;
        for (note_index, note) in block.notes() {
            locked_notes.insert(note.id(), note_tree.get_note_path(note_index).unwrap_or_default());
        }

        // update last block header
        *self.last_block_header.write().await = block.header();

        // update num_apply_block_called
        *self.num_apply_block_called.write().await += 1;

        Ok(())
    }
}

#[async_trait]
impl Store for MockStoreSuccess {
    async fn get_tx_inputs(
        &self,
        proven_tx: &ProvenTransaction,
    ) -> Result<TransactionInputs, TxInputsError> {
        let locked_accounts = self.accounts.read().await;
        let locked_produced_nullifiers = self.produced_nullifiers.read().await;

        let account_hash = {
            let account_hash = locked_accounts.get_leaf(&proven_tx.account_id().into());

            if account_hash == EMPTY_WORD {
                None
            } else {
                Some(account_hash.into())
            }
        };

        let nullifiers = proven_tx
            .input_notes()
            .iter()
            .map(|commitment| {
                let nullifier = commitment.nullifier();
                let nullifier_value = locked_produced_nullifiers.get_value(&nullifier.inner());

                (nullifier, nullifier_value[0].inner() as u32)
            })
            .collect();

        let locked_notes = self.notes.read().await;
        let missing_unauthenticated_notes = proven_tx
            .get_unauthenticated_notes()
            .filter_map(|header| {
                let id = header.id();
                locked_notes.contains_key(&id).not().then_some(id)
            })
            .collect();

        Ok(TransactionInputs {
            account_id: proven_tx.account_id(),
            account_hash,
            nullifiers,
            missing_unauthenticated_notes,
        })
    }

    async fn get_block_inputs(
        &self,
        updated_accounts: impl Iterator<Item = AccountId> + Send,
        produced_nullifiers: impl Iterator<Item = &Nullifier> + Send,
        notes: impl Iterator<Item = &NoteId> + Send,
    ) -> Result<BlockInputs, BlockInputsError> {
        let locked_accounts = self.accounts.read().await;
        let locked_produced_nullifiers = self.produced_nullifiers.read().await;

        let chain_peaks = {
            let locked_chain_mmr = self.chain_mmr.read().await;
            locked_chain_mmr.peaks(locked_chain_mmr.forest()).unwrap()
        };

        let accounts = {
            updated_accounts
                .map(|account_id| {
                    let ValuePath { value: hash, path: proof } =
                        locked_accounts.open(&account_id.into());

                    (account_id, AccountWitness { hash, proof })
                })
                .collect()
        };

        let nullifiers = produced_nullifiers
            .map(|nullifier| (*nullifier, locked_produced_nullifiers.open(&nullifier.inner())))
            .collect();

        let locked_notes = self.notes.read().await;
        let found_unauthenticated_notes =
            notes.filter(|&id| locked_notes.contains_key(id)).copied().collect();

        Ok(BlockInputs {
            block_header: *self.last_block_header.read().await,
            chain_peaks,
            accounts,
            nullifiers,
            found_unauthenticated_notes,
        })
    }

    async fn get_note_authentication_info(
        &self,
        notes: impl Iterator<Item = &NoteId> + Send,
    ) -> Result<BTreeMap<NoteId, MerklePath>, NotePathsError> {
        let locked_notes = self.notes.read().await;
        let note_auth_info = notes
            .map(|note_id| (*note_id, locked_notes.get(note_id).cloned().unwrap_or_default()))
            .collect();

        Ok(note_auth_info)
    }
}

#[derive(Default)]
pub struct MockStoreFailure;

#[async_trait]
impl ApplyBlock for MockStoreFailure {
    async fn apply_block(&self, _block: &Block) -> Result<(), ApplyBlockError> {
        Err(ApplyBlockError::GrpcClientError(String::new()))
    }
}

#[async_trait]
impl Store for MockStoreFailure {
    async fn get_tx_inputs(
        &self,
        _proven_tx: &ProvenTransaction,
    ) -> Result<TransactionInputs, TxInputsError> {
        Err(TxInputsError::Dummy)
    }

    async fn get_block_inputs(
        &self,
        _updated_accounts: impl Iterator<Item = AccountId> + Send,
        _produced_nullifiers: impl Iterator<Item = &Nullifier> + Send,
        _notes: impl Iterator<Item = &NoteId> + Send,
    ) -> Result<BlockInputs, BlockInputsError> {
        Err(BlockInputsError::GrpcClientError(String::new()))
    }

    async fn get_note_authentication_info(
        &self,
        _notes: impl Iterator<Item = &NoteId> + Send,
    ) -> Result<BTreeMap<NoteId, MerklePath>, NotePathsError> {
        Err(NotePathsError::GrpcClientError(String::new()))
    }
}
