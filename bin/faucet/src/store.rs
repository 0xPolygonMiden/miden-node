use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use anyhow::{bail, Context};
use miden_objects::{
    accounts::{Account, AccountId},
    crypto::hash::rpo::RpoDigest,
    notes::NoteId,
    transaction::{ChainMmr, InputNotes, TransactionInputs},
    utils::{Deserializable, Serializable},
    BlockHeader, Word,
};
use miden_tx::{DataStore, DataStoreError};

use crate::errors::{ErrorHelper, HandlerError};

const FAUCET_ACCOUNT_FILENAME: &str = "faucet-account.bin";

#[derive(Clone)]
pub struct FaucetDataStore {
    storage_path: PathBuf,
    faucet_account: Arc<RwLock<Account>>,
    seed: Word,
    block_header: BlockHeader,
    chain_mmr: ChainMmr,
}

// FAUCET DATA STORE
// ================================================================================================

impl FaucetDataStore {
    pub fn new(
        storage_path: PathBuf,
        faucet_account: Arc<RwLock<Account>>,
        seed: Word,
        root_block_header: BlockHeader,
        root_chain_mmr: ChainMmr,
    ) -> Self {
        Self {
            storage_path,
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

    /// Saves the next faucet state to file.
    pub async fn save_next_faucet_state(
        &self,
        next_faucet_state: &Account,
    ) -> Result<(), HandlerError> {
        tokio::fs::create_dir_all(&self.storage_path)
            .await
            .or_fail("Failed to create directory for storage")?;

        tokio::fs::write(next_faucet_state_path(&self.storage_path), next_faucet_state.to_bytes())
            .await
            .or_fail("Failed to save faucet account to file")
    }

    /// Switches file storage to the next faucet state.
    pub async fn switch_to_next_faucet_state(&self) -> Result<(), HandlerError> {
        tokio::fs::rename(
            next_faucet_state_path(&self.storage_path),
            faucet_state_path(&self.storage_path),
        )
        .await
        .or_fail("Failed to rename next faucet account file to the current one")
    }

    /// Updates the stored faucet account with the new one.
    pub async fn update_faucet_state(
        &mut self,
        new_faucet_state: Account,
    ) -> Result<(), HandlerError> {
        self.switch_to_next_faucet_state().await?;
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

// HELPER FUNCTIONS
// ================================================================================================

/// Tries to restore the faucet state from current and/or next file(s).
///
/// If the faucet state with the expected hash is not found in either file, an error is returned.
pub async fn resolve_faucet_state(
    storage_path: impl AsRef<Path>,
    expected_hash: RpoDigest,
) -> anyhow::Result<Account> {
    if !faucet_state_path(&storage_path).exists() {
        bail!("Faucet state file does not exist");
    }

    let current_state = load_current_faucet_state(&storage_path)
        .await
        .context("Failed to restore current faucet state")?;

    if current_state.hash() == expected_hash {
        return Ok(current_state);
    }

    if !next_faucet_state_path(&storage_path).exists() {
        bail!("Next faucet state file does not exist");
    }

    let next_state = load_next_faucet_state(&storage_path)
        .await
        .context("Failed to restore next faucet state")?;

    if next_state.hash() == expected_hash {
        return Ok(next_state);
    }

    bail!(
        "Failed to restore faucet state from files. Could not find file with expected state hash."
    );
}

/// Loads the current faucet state from file.
pub async fn load_current_faucet_state(storage_path: impl AsRef<Path>) -> anyhow::Result<Account> {
    load_faucet_state_internal(faucet_state_path(storage_path)).await
}

/// Loads the next faucet state from file.
pub async fn load_next_faucet_state(storage_path: impl AsRef<Path>) -> anyhow::Result<Account> {
    load_faucet_state_internal(next_faucet_state_path(storage_path)).await
}

/// Loads the faucet state from file with the given path.
async fn load_faucet_state_internal(path: impl AsRef<Path>) -> anyhow::Result<Account> {
    let bytes = tokio::fs::read(path).await.context("Failed to read faucet account from file")?;

    Account::read_from_bytes(&bytes)
        .map_err(|err| anyhow::anyhow!("Failed to deserialize faucet account from bytes: {err}"))
}

/// Returns path to the faucet state file.
fn faucet_state_path(storage_path: impl AsRef<Path>) -> PathBuf {
    storage_path.as_ref().join(FAUCET_ACCOUNT_FILENAME)
}

/// Returns path to the next faucet state file.
fn next_faucet_state_path(storage_path: impl AsRef<Path>) -> PathBuf {
    faucet_state_path(storage_path).with_extension("next")
}
