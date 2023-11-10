use async_trait::async_trait;
use miden_node_proto::domain::BlockInputs;
use miden_objects::EMPTY_WORD;
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
        let accounts: Vec<_> = accounts
            .into_iter()
            .map(|(account_id, hash)| (account_id.into(), hash.into()))
            .collect();
        let store_accounts = SimpleSmt::with_leaves(64, accounts).unwrap();

        Self {
            accounts: Arc::new(RwLock::new(store_accounts)),
            consumed_nullifiers: Arc::new(RwLock::new(consumed_nullifiers)),
            num_apply_block_called: Arc::new(RwLock::new(0)),
        }
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
        _updated_accounts: impl Iterator<Item = &AccountId> + Send,
        _produced_nullifiers: impl Iterator<Item = &Digest> + Send,
    ) -> Result<BlockInputs, BlockInputsError> {
        unimplemented!()
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
