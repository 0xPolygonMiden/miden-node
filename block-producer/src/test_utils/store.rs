use async_trait::async_trait;
use miden_air::{Felt, FieldElement};
use miden_node_proto::domain::{AccountInputRecord, BlockInputs};
use miden_objects::{
    crypto::merkle::{Mmr, MmrPeaks},
    BlockHeader, EMPTY_WORD, ZERO,
};
use miden_vm::crypto::SimpleSmt;

use crate::{
    block::Block,
    store::{ApplyBlock, ApplyBlockError, BlockInputsError, Store, TxInputs, TxInputsError},
    SharedProvenTx,
};

use super::*;

const ACCOUNT_SMT_DEPTH: u8 = 64;

/// Builds a [`MockStoreSuccess`]
#[derive(Debug, Default)]
pub struct MockStoreSuccessBuilder {
    accounts: Option<SimpleSmt>,
    consumed_nullifiers: Option<BTreeSet<Digest>>,
    chain_mmr: Option<Mmr>,
    block_header: Option<BlockHeader>,
}

impl MockStoreSuccessBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initial_accounts(
        mut self,
        accounts: impl Iterator<Item = (AccountId, Digest)>,
    ) -> Self {
        let accounts_smt = {
            let accounts =
                accounts.into_iter().map(|(account_id, hash)| (account_id.into(), hash.into()));

            SimpleSmt::with_leaves(ACCOUNT_SMT_DEPTH, accounts).unwrap()
        };

        self.accounts = Some(accounts_smt);

        self
    }

    pub fn initial_nullifiers(
        mut self,
        consumed_nullifiers: BTreeSet<Digest>,
    ) -> Self {
        self.consumed_nullifiers = Some(consumed_nullifiers);

        self
    }

    pub fn initial_chain_mmr(
        mut self,
        chain_mmr: Mmr,
    ) -> Self {
        self.chain_mmr = Some(chain_mmr);

        self
    }

    pub fn initial_block_header(
        mut self,
        block_header: BlockHeader,
    ) -> Self {
        self.block_header = Some(block_header);

        self
    }

    pub fn build(self) -> MockStoreSuccess {
        let default_block_header = || {
            BlockHeader::new(
                Digest::default(),
                Felt::ZERO,
                Digest::default(),
                Digest::default(),
                Digest::default(),
                Digest::default(),
                Digest::default(),
                Digest::default(),
                Felt::ZERO,
                Felt::ONE,
            )
        };

        MockStoreSuccess {
            accounts: Arc::new(RwLock::new(
                self.accounts.unwrap_or(SimpleSmt::new(ACCOUNT_SMT_DEPTH).unwrap()),
            )),
            consumed_nullifiers: Arc::new(RwLock::new(
                self.consumed_nullifiers.unwrap_or_default(),
            )),
            chain_mmr: Arc::new(RwLock::new(self.chain_mmr.unwrap_or_default())),
            last_block_header: Arc::new(RwLock::new(
                self.block_header.unwrap_or_else(default_block_header),
            )),
            num_apply_block_called: Arc::new(RwLock::new(0)),
        }
    }
}

pub struct MockStoreSuccess {
    /// Map account id -> account hash
    accounts: Arc<RwLock<SimpleSmt>>,

    /// Stores the nullifiers of the notes that were consumed
    consumed_nullifiers: Arc<RwLock<BTreeSet<Digest>>>,

    // Stores the chain MMR
    chain_mmr: Arc<RwLock<Mmr>>,

    // Stores the header of the last applied block
    last_block_header: Arc<RwLock<BlockHeader>>,

    /// The number of times `apply_block()` was called
    pub num_apply_block_called: Arc<RwLock<u32>>,
}

impl MockStoreSuccess {
    /// Update some accounts in the store
    pub async fn update_accounts(
        &self,
        updated_accounts: impl Iterator<Item = (AccountId, Digest)>,
    ) {
        let mut locked_accounts = self.accounts.write().await;
        for (account_id, new_account_state) in updated_accounts {
            locked_accounts
                .update_leaf(account_id.into(), new_account_state.into())
                .unwrap();
        }
    }

    pub async fn account_root(&self) -> Digest {
        let locked_accounts = self.accounts.read().await;

        locked_accounts.root()
    }
}

#[async_trait]
impl ApplyBlock for MockStoreSuccess {
    async fn apply_block(
        &self,
        block: Arc<Block>,
    ) -> Result<(), ApplyBlockError> {
        // Intentionally, we take and hold both locks, to prevent calls to `get_tx_inputs()` from going through while we're updating the store's data structure
        let mut locked_accounts = self.accounts.write().await;
        let mut locked_consumed_nullifiers = self.consumed_nullifiers.write().await;

        // update accounts
        for &(account_id, account_hash) in block.updated_accounts.iter() {
            locked_accounts.update_leaf(account_id.into(), account_hash.into()).unwrap();
        }

        // update nullifiers
        let mut new_nullifiers: BTreeSet<Digest> =
            block.produced_nullifiers.iter().cloned().collect();
        locked_consumed_nullifiers.append(&mut new_nullifiers);

        // update chain mmr with new block header hash
        {
            let mut chain_mmr = self.chain_mmr.write().await;

            chain_mmr.add(block.header.hash());
        }

        // update num_apply_block_called
        *self.num_apply_block_called.write().await += 1;

        Ok(())
    }
}

#[async_trait]
impl Store for MockStoreSuccess {
    async fn get_tx_inputs(
        &self,
        proven_tx: SharedProvenTx,
    ) -> Result<TxInputs, TxInputsError> {
        let locked_accounts = self.accounts.read().await;
        let locked_consumed_nullifiers = self.consumed_nullifiers.read().await;

        let account_hash = {
            let account_hash = locked_accounts.get_leaf(proven_tx.account_id().into()).unwrap();

            if account_hash == EMPTY_WORD {
                None
            } else {
                Some(account_hash.into())
            }
        };

        let nullifiers = proven_tx
            .consumed_notes()
            .iter()
            .map(|note| (note.nullifier(), locked_consumed_nullifiers.contains(&note.nullifier())))
            .collect();

        Ok(TxInputs {
            account_hash,
            nullifiers,
        })
    }

    async fn get_block_inputs(
        &self,
        updated_accounts: impl Iterator<Item = &AccountId> + Send,
        _produced_nullifiers: impl Iterator<Item = &Digest> + Send,
    ) -> Result<BlockInputs, BlockInputsError> {
        let block_header = {
            let prev_hash: Digest = Digest::default();
            let chain_root = {
                let chain_mmr = self.chain_mmr.read().await;

                chain_mmr.peaks(chain_mmr.forest()).unwrap().hash_peaks()
            };
            let account_root: Digest = self.account_root().await;
            let nullifier_root: Digest = Digest::default();
            let note_root: Digest = Digest::default();
            let batch_root: Digest = Digest::default();
            let proof_hash: Digest = Digest::default();

            BlockHeader::new(
                prev_hash,
                Felt::ZERO,
                chain_root,
                account_root,
                nullifier_root,
                note_root,
                batch_root,
                proof_hash,
                Felt::ZERO,
                Felt::ONE,
            )
        };

        let chain_peaks = MmrPeaks::new(0, Vec::new()).unwrap();

        let account_states = {
            let locked_accounts = self.accounts.read().await;

            updated_accounts
                .map(|&account_id| {
                    let account_hash = locked_accounts.get_leaf(account_id.into()).unwrap();
                    let proof = locked_accounts.get_leaf_path(account_id.into()).unwrap();

                    AccountInputRecord {
                        account_id,
                        account_hash: account_hash.into(),
                        proof,
                    }
                })
                .collect()
        };

        Ok(BlockInputs {
            block_header,
            chain_peaks,
            account_states,
            // TODO: return a proper nullifiers iterator
            nullifiers: Vec::new(),
        })
    }
}

#[derive(Default)]
pub struct MockStoreFailure;

#[async_trait]
impl ApplyBlock for MockStoreFailure {
    async fn apply_block(
        &self,
        _block: Arc<Block>,
    ) -> Result<(), ApplyBlockError> {
        Err(ApplyBlockError::Dummy)
    }
}

#[async_trait]
impl Store for MockStoreFailure {
    async fn get_tx_inputs(
        &self,
        _proven_tx: SharedProvenTx,
    ) -> Result<TxInputs, TxInputsError> {
        Err(TxInputsError::Dummy)
    }

    async fn get_block_inputs(
        &self,
        _updated_accounts: impl Iterator<Item = &AccountId> + Send,
        _produced_nullifiers: impl Iterator<Item = &Digest> + Send,
    ) -> Result<BlockInputs, BlockInputsError> {
        Err(BlockInputsError::Dummy)
    }
}
