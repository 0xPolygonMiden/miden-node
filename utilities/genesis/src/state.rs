use miden_objects::accounts::Account;
use serde::{Deserialize, Serialize};

/// Represents the state at genesis, which will be used to derive the genesis block.
#[derive(Serialize, Deserialize)]
pub struct GenesisState {
    pub accounts: Vec<Account>,
}

impl Default for GenesisState {
    fn default() -> Self {
        let accounts = Vec::new();

        Self { accounts }
    }
}
