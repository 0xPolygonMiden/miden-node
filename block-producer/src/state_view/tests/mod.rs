use super::*;

use std::collections::BTreeMap;

use miden_objects::{
    accounts::get_account_seed,
    transaction::{ConsumedNoteInfo, ProvenTransaction},
    Hasher,
};

use crate::{store::TxInputsError, test_utils::DummyProvenTxGenerator};

mod verify_tx;

// MOCK STORES
// -------------------------------------------------------------------------------------------------

#[derive(Default)]
struct MockStoreSuccess {
    /// Map account id -> account hash
    accounts: Arc<RwLock<BTreeMap<AccountId, Digest>>>,

    /// Stores the nullifiers of the notes that were consumed
    consumed_nullifiers: Arc<RwLock<BTreeSet<Digest>>>,
}

impl MockStoreSuccess {
    /// Initializes the known accounts from provided mock accounts, where the account hash in the
    /// store is the first state in `MockAccount.states`.
    fn new(
        accounts: impl Iterator<Item = MockAccount>,
        consumed_nullifiers: BTreeSet<Digest>,
    ) -> Self {
        let store_accounts: BTreeMap<AccountId, Digest> =
            accounts.map(|account| (account.id, account.states[0])).collect();

        Self {
            accounts: Arc::new(RwLock::new(store_accounts)),
            consumed_nullifiers: Arc::new(RwLock::new(consumed_nullifiers)),
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

// HELPERS
// -------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct MockAccount {
    pub id: AccountId,

    // Sequence of 3 states that the account goes into
    pub states: [Digest; 3],
}

impl MockAccount {
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

impl From<u8> for MockAccount {
    /// Each index gives rise to a different account ID
    fn from(index: u8) -> Self {
        let mut init_seed: [u8; 32] = [0; 32];
        init_seed[0] = index;

        Self::new(init_seed)
    }
}

pub fn consumed_note_by_index(index: u8) -> ConsumedNoteInfo {
    ConsumedNoteInfo::new(Hasher::hash(&[index]), Hasher::hash(&[index, index]))
}

/// Returns `num` transactions, and the corresponding account they modify.
/// The transactions each consume a single different note
pub fn get_txs_and_accounts<'a>(
    tx_gen: &'a DummyProvenTxGenerator,
    num: u8,
) -> impl Iterator<Item = (ProvenTransaction, MockAccount)> + 'a {
    (0..num).map(|index| {
        let account = MockAccount::from(index);
        let tx = tx_gen.dummy_proven_tx_with_params(
            account.id,
            account.states[0],
            account.states[1],
            vec![consumed_note_by_index(index)],
        );

        (tx, account)
    })
}
