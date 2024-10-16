use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU32,
    ops::Not,
};

use async_trait::async_trait;
use miden_node_proto::domain::{blocks::BlockInclusionProof, notes::NoteAuthenticationInfo};
use miden_objects::{
    block::{Block, NoteBatch},
    crypto::merkle::{Mmr, SimpleSmt, Smt, ValuePath},
    notes::{NoteId, NoteInclusionProof, Nullifier},
    BlockHeader, ACCOUNT_TREE_DEPTH, EMPTY_WORD, ZERO,
};
use tokio::sync::RwLock;

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
    notes: Option<Vec<NoteBatch>>,
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

        Self {
            accounts: Some(accounts_smt),
            notes: Some(block_output_notes(batches_iter).cloned().collect()),
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
            produced_nullifiers: None,
            chain_mmr: None,
            block_num: None,
        }
    }

    pub fn initial_notes<'a>(mut self, notes: impl Iterator<Item = &'a NoteBatch> + Clone) -> Self {
        self.notes = Some(notes.cloned().collect());

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

    pub fn build(self) -> MockStoreSuccess {
        let block_num = self.block_num.unwrap_or(1);
        let accounts_smt = self.accounts.unwrap_or(SimpleSmt::new().unwrap());
        let notes = self.notes.unwrap_or_default();
        let block_note_tree = note_created_smt_from_note_batches(notes.iter());
        let note_root = block_note_tree.root();
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
            Digest::default(),
            1,
        );

        let notes = flatten_output_notes(notes.iter())
            .map(|(index, note)| {
                (
                    note.id(),
                    NoteInclusionProof::new(
                        block_num,
                        index.leaf_index_value(),
                        block_note_tree.get_note_path(index),
                    )
                    .expect("Failed to create `NoteInclusionProof`"),
                )
            })
            .collect();

        MockStoreSuccess {
            accounts: Arc::new(RwLock::new(accounts_smt)),
            produced_nullifiers: Arc::new(RwLock::new(nullifiers_smt)),
            chain_mmr: Arc::new(RwLock::new(chain_mmr)),
            block_headers: Arc::new(RwLock::new(BTreeMap::from_iter([(
                initial_block_header.block_num(),
                initial_block_header,
            )]))),
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

    /// The chains block headers.
    pub block_headers: Arc<RwLock<BTreeMap<u32, BlockHeader>>>,

    /// The number of times `apply_block()` was called
    pub num_apply_block_called: Arc<RwLock<u32>>,

    /// Maps note id -> note inclusion proof for all created notes
    pub notes: Arc<RwLock<BTreeMap<NoteId, NoteInclusionProof>>>,
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
        // Intentionally, we take and hold both locks, to prevent calls to `get_tx_inputs()` from
        // going through while we're updating the store's data structure
        let mut locked_accounts = self.accounts.write().await;
        let mut locked_produced_nullifiers = self.produced_nullifiers.write().await;

        // update accounts
        for update in block.updated_accounts() {
            locked_accounts.insert(update.account_id().into(), update.new_state_hash().into());
        }
        let header = block.header();
        debug_assert_eq!(locked_accounts.root(), header.account_root());

        // update nullifiers
        for nullifier in block.nullifiers() {
            locked_produced_nullifiers
                .insert(nullifier.inner(), [header.block_num().into(), ZERO, ZERO, ZERO]);
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
            locked_notes.insert(
                note.id(),
                NoteInclusionProof::new(
                    header.block_num(),
                    note_index.leaf_index_value(),
                    note_tree.get_note_path(note_index),
                )
                .expect("Failed to build `NoteInclusionProof`"),
            );
        }

        // append the block header
        self.block_headers.write().await.insert(header.block_num(), header);

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

                (nullifier, NonZeroU32::new(nullifier_value[0].inner() as u32))
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
            current_block_height: 0,
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
        let note_proofs = notes
            .filter_map(|id| locked_notes.get(id).map(|proof| (*id, proof.clone())))
            .collect::<BTreeMap<_, _>>();

        let locked_headers = self.block_headers.read().await;
        let latest_header =
            *locked_headers.iter().max_by_key(|(block_num, _)| *block_num).unwrap().1;

        let locked_chain_mmr = self.chain_mmr.read().await;
        let mmr_forest = locked_chain_mmr.forest();
        let chain_length = latest_header.block_num();
        let block_proofs = note_proofs
            .values()
            .map(|note_proof| {
                let block_num = note_proof.location().block_num();
                let block_header = *locked_headers.get(&block_num).unwrap();
                let mmr_path =
                    locked_chain_mmr.open(block_num as usize, mmr_forest).unwrap().merkle_path;

                (block_num, BlockInclusionProof { block_header, mmr_path, chain_length })
            })
            .collect();

        let found_unauthenticated_notes = NoteAuthenticationInfo { block_proofs, note_proofs };

        Ok(BlockInputs {
            block_header: latest_header,
            chain_peaks,
            accounts,
            nullifiers,
            found_unauthenticated_notes,
        })
    }

    async fn get_note_authentication_info(
        &self,
        notes: impl Iterator<Item = &NoteId> + Send,
    ) -> Result<NoteAuthenticationInfo, NotePathsError> {
        let locked_notes = self.notes.read().await;
        let locked_headers = self.block_headers.read().await;
        let locked_chain_mmr = self.chain_mmr.read().await;

        let note_proofs = notes
            .filter_map(|id| locked_notes.get(id).map(|proof| (*id, proof.clone())))
            .collect::<BTreeMap<_, _>>();

        let latest_header =
            *locked_headers.iter().max_by_key(|(block_num, _)| *block_num).unwrap().1;
        let chain_length = latest_header.block_num();

        let block_proofs = note_proofs
            .values()
            .map(|note_proof| {
                let block_num = note_proof.location().block_num();
                let block_header = *locked_headers.get(&block_num).unwrap();
                let mmr_path = locked_chain_mmr
                    .open(block_num as usize, latest_header.block_num() as usize)
                    .unwrap()
                    .merkle_path;

                (block_num, BlockInclusionProof { block_header, mmr_path, chain_length })
            })
            .collect();

        Ok(NoteAuthenticationInfo { block_proofs, note_proofs })
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
    ) -> Result<NoteAuthenticationInfo, NotePathsError> {
        Err(NotePathsError::GrpcClientError(String::new()))
    }
}
