use std::sync::{Arc, RwLock};

use miden_objects::{
    accounts::{Account, AccountId},
    notes::NoteId,
    transaction::{ChainMmr, InputNotes, TransactionInputs},
    BlockHeader, Word,
};
use miden_tx::{DataStore, DataStoreError};

use crate::errors::HandlerError;

#[derive(Clone)]
pub struct FaucetDataStore {
    faucet_account: Arc<RwLock<Account>>,
    /// Seed used for faucet account creation.
    seed: Word,
    block_header: BlockHeader,
    chain_mmr: ChainMmr,
}

// FAUCET DATA STORE
// ================================================================================================

impl FaucetDataStore {
    pub fn new(
        faucet_account: Arc<RwLock<Account>>,
        seed: Word,
        root_block_header: BlockHeader,
        root_chain_mmr: ChainMmr,
    ) -> Self {
        Self {
            faucet_account,
            seed,
            block_header: root_block_header,
            chain_mmr: root_chain_mmr,
        }
    }

    /// Returns the stored faucet account.
    pub fn faucet_account(&self) -> Account {
        self.faucet_account.read().expect("Poisoned lock").clone()
    }

    /// Updates the stored faucet account with the new one.
    pub async fn update_faucet_state(&self, new_faucet_state: Account) -> Result<(), HandlerError> {
        *self.faucet_account.write().expect("Poisoned lock") = new_faucet_state;

        Ok(())
    }
}

impl DataStore for FaucetDataStore {
    fn get_transaction_inputs(
        &self,
        account_id: AccountId,
        _block_ref: u32,
        _notes: &[NoteId],
    ) -> Result<TransactionInputs, DataStoreError> {
        let account = self.faucet_account.read().expect("Poisoned lock");
        if account_id != account.id() {
            return Err(DataStoreError::AccountNotFound(account_id));
        }

        let empty_input_notes =
            InputNotes::new(Vec::new()).map_err(DataStoreError::InvalidTransactionInput)?;

        TransactionInputs::new(
            account.clone(),
            account.is_new().then_some(self.seed),
            self.block_header,
            self.chain_mmr.clone(),
            empty_input_notes,
        )
        .map_err(DataStoreError::InvalidTransactionInput)
    }
}
