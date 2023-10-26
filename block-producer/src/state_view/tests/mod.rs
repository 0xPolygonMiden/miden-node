use super::*;

use std::collections::BTreeMap;

use miden_objects::{
    accounts::get_account_seed,
    transaction::{ConsumedNoteInfo, ProvenTransaction},
    BlockHeader, Felt, Hasher,
};

use crate::{store::TxInputsError, test_utils::DummyProvenTxGenerator};

mod apply_block;
mod verify_tx;

// MOCK STORES
// -------------------------------------------------------------------------------------------------

#[derive(Default)]
struct MockStoreSuccess {
    /// Map account id -> account hash
    accounts: Arc<RwLock<BTreeMap<AccountId, Digest>>>,

    /// Stores the nullifiers of the notes that were consumed
    consumed_nullifiers: Arc<RwLock<BTreeSet<Digest>>>,

    /// The number of times `apply_block()` was called
    num_apply_block_called: Arc<RwLock<u32>>,
}

impl MockStoreSuccess {
    /// Initializes the known accounts from provided mock accounts, where the account hash in the
    /// store is the first state in `MockAccount.states`.
    fn new(
        accounts: impl Iterator<Item = MockPrivateAccount>,
        consumed_nullifiers: BTreeSet<Digest>,
    ) -> Self {
        let store_accounts: BTreeMap<AccountId, Digest> =
            accounts.map(|account| (account.id, account.states[0])).collect();

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
            locked_accounts.insert(account_id, account_hash);
        }

        let mut new_nullifiers: BTreeSet<Digest> = block.new_nullifiers.iter().cloned().collect();
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

// MOCK PRIVATE ACCOUNT
// -------------------------------------------------------------------------------------------------

/// A mock representation fo private accounts. An account starts in state `states[0]`, is modified
/// to state `states[1]`, and so on.
#[derive(Clone, Copy, Debug)]
pub struct MockPrivateAccount<const NUM_STATES: usize = 3> {
    pub id: AccountId,

    // Sequence states that the account goes into.
    pub states: [Digest; NUM_STATES],
}

impl<const NUM_STATES: usize> MockPrivateAccount<NUM_STATES> {
    fn new(init_seed: [u8; 32]) -> Self {
        let account_seed = get_account_seed(
            init_seed,
            miden_objects::accounts::AccountType::RegularAccountUpdatableCode,
            false,
            Digest::default(),
            Digest::default(),
        )
        .unwrap();

        let mut states = [Digest::default(); NUM_STATES];

        states[0] = Hasher::hash(&init_seed);
        for idx in 1..NUM_STATES {
            states[idx] = Hasher::hash(&states[idx - 1].as_bytes());
        }

        Self {
            id: AccountId::new(account_seed, Digest::default(), Digest::default()).unwrap(),
            states,
        }
    }
}

impl<const NUM_STATES: usize> From<u8> for MockPrivateAccount<NUM_STATES> {
    /// Each index gives rise to a different account ID
    fn from(index: u8) -> Self {
        let mut init_seed: [u8; 32] = [0; 32];
        init_seed[0] = index;

        Self::new(init_seed)
    }
}

// HELPERS
// -------------------------------------------------------------------------------------------------

pub fn consumed_note_by_index(index: u8) -> ConsumedNoteInfo {
    ConsumedNoteInfo::new(Hasher::hash(&[index]), Hasher::hash(&[index, index]))
}

/// Returns `num` transactions, and the corresponding account they modify.
/// The transactions each consume a single different note
pub fn get_txs_and_accounts<'a>(
    tx_gen: &'a DummyProvenTxGenerator,
    num: u8,
) -> impl Iterator<Item = (ProvenTransaction, MockPrivateAccount)> + 'a {
    (0..num).map(|index| {
        let account = MockPrivateAccount::from(index);
        let tx = tx_gen.dummy_proven_tx_with_params(
            account.id,
            account.states[0],
            account.states[1],
            vec![consumed_note_by_index(index)],
        );

        (tx, account)
    })
}

pub fn get_dummy_block(
    updated_accounts: Vec<MockPrivateAccount>,
    new_nullifiers: Vec<Digest>,
) -> Block {
    let header = BlockHeader::new(
        Digest::default(),
        Felt::new(42),
        Digest::default(),
        Digest::default(),
        Digest::default(),
        Digest::default(),
        Digest::default(),
        Digest::default(),
        Felt::new(0),
        Felt::new(42),
    );

    let updated_accounts = updated_accounts
        .into_iter()
        .map(|mock_account| (mock_account.id, mock_account.states[1]))
        .collect();

    Block {
        header,
        updated_accounts,
        created_notes: Vec::new(),
        new_nullifiers,
    }
}
