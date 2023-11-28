use async_trait::async_trait;
use miden_air::{Felt, FieldElement};
use miden_node_proto::domain::{AccountInputRecord, BlockInputs};
use miden_objects::{crypto::merkle::MmrPeaks, BlockHeader, EMPTY_WORD};
use miden_vm::crypto::SimpleSmt;

use crate::{
    block::Block,
    store::{ApplyBlock, ApplyBlockError, BlockInputsError, Store, TxInputs, TxInputsError},
    SharedProvenTx,
};

use super::*;

pub struct MockStoreSuccess {
    /// Map account id -> account hash
    accounts: Arc<RwLock<SimpleSmt>>,

    /// Stores the nullifiers of the notes that were consumed
    consumed_nullifiers: Arc<RwLock<BTreeSet<Digest>>>,

    /// The number of times `apply_block()` was called
    pub num_apply_block_called: Arc<RwLock<u32>>,
}

impl MockStoreSuccess {
    /// Initializes the known accounts from provided mock accounts, where the account hash in the
    /// store is the first state in `MockAccount.states`.
    pub fn new(
        accounts: impl Iterator<Item = (AccountId, Digest)>,
        consumed_nullifiers: BTreeSet<Digest>,
    ) -> Self {
        let accounts =
            accounts.into_iter().map(|(account_id, hash)| (account_id.into(), hash.into()));
        let store_accounts = SimpleSmt::with_leaves(64, accounts).unwrap();

        Self {
            accounts: Arc::new(RwLock::new(store_accounts)),
            consumed_nullifiers: Arc::new(RwLock::new(consumed_nullifiers)),
            num_apply_block_called: Arc::new(RwLock::new(0)),
        }
    }

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

        for &(account_id, account_hash) in block.updated_accounts.iter() {
            locked_accounts.update_leaf(account_id.into(), account_hash.into()).unwrap();
        }

        let mut new_nullifiers: BTreeSet<Digest> =
            block.produced_nullifiers.iter().cloned().collect();
        locked_consumed_nullifiers.append(&mut new_nullifiers);

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
            let chain_root: Digest = Digest::default();
            let acct_root: Digest = self.account_root().await;
            let nullifier_root: Digest = Digest::default();
            let note_root: Digest = Digest::default();
            let batch_root: Digest = Digest::default();
            let proof_hash: Digest = Digest::default();

            BlockHeader::new(
                prev_hash,
                Felt::ZERO,
                chain_root,
                acct_root,
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
