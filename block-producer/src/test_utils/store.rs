use async_trait::async_trait;
use miden_node_proto::domain::{AccountInputRecord, BlockInputs};
use miden_objects::{crypto::merkle::Mmr, BlockHeader, ACCOUNT_TREE_DEPTH, EMPTY_WORD, ONE, ZERO};
use miden_vm::crypto::SimpleSmt;

use super::*;
use crate::{
    block::Block,
    store::{ApplyBlock, ApplyBlockError, BlockInputsError, Store, TxInputs, TxInputsError},
    SharedProvenTx,
};

/// Builds a [`MockStoreSuccess`]
#[derive(Debug, Default)]
pub struct MockStoreSuccessBuilder {
    accounts: Option<SimpleSmt<ACCOUNT_TREE_DEPTH>>,
    consumed_nullifiers: Option<BTreeSet<Digest>>,
    chain_mmr: Option<Mmr>,
}

impl MockStoreSuccessBuilder {
    /// FIXME: the store always needs to be properly initialized with initial accounts
    /// see https://github.com/0xPolygonMiden/miden-node/issues/79
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

            SimpleSmt::<ACCOUNT_TREE_DEPTH>::with_leaves(accounts).unwrap()
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

    pub fn build(self) -> MockStoreSuccess {
        let accounts_smt = self.accounts.unwrap_or(SimpleSmt::<ACCOUNT_TREE_DEPTH>::new().unwrap());
        let chain_mmr = self.chain_mmr.unwrap_or_default();

        let initial_block_header = BlockHeader::new(
            Digest::default(),
            0,
            chain_mmr.peaks(chain_mmr.forest()).unwrap().hash_peaks(),
            accounts_smt.root(),
            Digest::default(),
            // FIXME: FILL IN CORRECT VALUE
            Digest::default(),
            Digest::default(),
            Digest::default(),
            ZERO,
            ONE,
        );

        MockStoreSuccess {
            accounts: Arc::new(RwLock::new(accounts_smt)),
            consumed_nullifiers: Arc::new(RwLock::new(
                self.consumed_nullifiers.unwrap_or_default(),
            )),
            chain_mmr: Arc::new(RwLock::new(chain_mmr)),
            last_block_header: Arc::new(RwLock::new(initial_block_header)),
            num_apply_block_called: Arc::new(RwLock::new(0)),
        }
    }
}

pub struct MockStoreSuccess {
    /// Map account id -> account hash
    pub accounts: Arc<RwLock<SimpleSmt<ACCOUNT_TREE_DEPTH>>>,

    /// Stores the nullifiers of the notes that were consumed
    pub consumed_nullifiers: Arc<RwLock<BTreeSet<Digest>>>,

    // Stores the chain MMR
    pub chain_mmr: Arc<RwLock<Mmr>>,

    // Stores the header of the last applied block
    pub last_block_header: Arc<RwLock<BlockHeader>>,

    /// The number of times `apply_block()` was called
    pub num_apply_block_called: Arc<RwLock<u32>>,
}

impl MockStoreSuccess {
    pub async fn account_root(&self) -> Digest {
        let locked_accounts = self.accounts.read().await;

        locked_accounts.root()
    }
}

#[async_trait]
impl ApplyBlock for MockStoreSuccess {
    async fn apply_block(
        &self,
        block: Block,
    ) -> Result<(), ApplyBlockError> {
        // Intentionally, we take and hold both locks, to prevent calls to `get_tx_inputs()` from going through while we're updating the store's data structure
        let mut locked_accounts = self.accounts.write().await;
        let mut locked_consumed_nullifiers = self.consumed_nullifiers.write().await;

        // update accounts
        for &(account_id, account_hash) in block.updated_accounts.iter() {
            locked_accounts.update_leaf(account_id.into(), account_hash.into()).unwrap();
        }
        debug_assert_eq!(locked_accounts.root(), block.header.account_root());

        // update nullifiers
        let mut new_nullifiers: BTreeSet<Digest> =
            block.produced_nullifiers.iter().cloned().collect();
        locked_consumed_nullifiers.append(&mut new_nullifiers);

        // update chain mmr with new block header hash
        {
            let mut chain_mmr = self.chain_mmr.write().await;

            chain_mmr.add(block.header.hash());
        }

        // update last block header
        *self.last_block_header.write().await = block.header;

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
            .input_notes()
            .iter()
            .map(|nullifier| {
                (nullifier.inner(), locked_consumed_nullifiers.contains(&nullifier.inner()))
            })
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
        let chain_peaks = {
            let locked_chain_mmr = self.chain_mmr.read().await;
            locked_chain_mmr.peaks(locked_chain_mmr.forest()).unwrap()
        };

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
            block_header: *self.last_block_header.read().await,
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
        _block: Block,
    ) -> Result<(), ApplyBlockError> {
        Err(ApplyBlockError::GrpcClientError(String::new()))
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
        Err(BlockInputsError::GrpcClientError(String::new()))
    }
}
