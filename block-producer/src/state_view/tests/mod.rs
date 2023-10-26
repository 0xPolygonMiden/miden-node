use std::collections::BTreeMap;

use miden_objects::{accounts::get_account_seed, Hasher};

use super::*;
use crate::store::TxInputsError;

// MOCK STORES
// -------------------------------------------------------------------------------------------------

#[derive(Default)]
struct MockStoreSuccess {
    /// Map account id -> account hash
    accounts: Arc<RwLock<BTreeMap<AccountId, Digest>>>,

    /// Stores the nullifiers of the notes that were consumed
    consumed_nullifiers: Arc<RwLock<BTreeSet<Digest>>>,
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
            locked_accounts.insert(account_id, account_hash);
        }

        let mut new_nullifiers: BTreeSet<Digest> = block.new_nullifiers.iter().cloned().collect();
        locked_consumed_nullifiers.append(&mut new_nullifiers);

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

        let account_hash = locked_accounts.get(&proven_tx.account_id()).cloned();

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
}

#[derive(Default)]
struct MockStoreFailure;

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
}

// MOCK ACCOUNT
// -------------------------------------------------------------------------------------------------

pub struct MockAccount {
    pub id: AccountId,

    // Sequence of 3 states that the account goes into
    pub states: [Digest; 3],
}

impl MockAccount {
    pub fn account_1() -> Self {
        let mut init_seed: [u8; 32] = [0; 32];
        init_seed[0] = 42;

        Self::new(init_seed)
    }

    pub fn account_2() -> Self {
        let mut init_seed: [u8; 32] = [0; 32];
        init_seed[0] = 43;

        Self::new(init_seed)
    }

    pub fn account_3() -> Self {
        let mut init_seed: [u8; 32] = [0; 32];
        init_seed[0] = 44;

        Self::new(init_seed)
    }

    fn new(init_seed: [u8; 32]) -> Self {
        let account_seed = get_account_seed(
            init_seed,
            miden_objects::accounts::AccountType::RegularAccountUpdatableCode,
            false,
            Digest::default(),
            Digest::default(),
        )
        .unwrap();

        let state_0 = Hasher::hash(&init_seed);
        let state_1 = Hasher::hash(&state_0.as_bytes());
        let state_2 = Hasher::hash(&state_1.as_bytes());

        Self {
            id: AccountId::new(account_seed, Digest::default(), Digest::default()).unwrap(),
            states: [state_0, state_1, state_2],
        }
    }
}
