use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

// INPUT HELPER STRUCTS
// ================================================================================================

/// Input types are helper structures designed for parsing and deserializing genesis input files.
/// They serve as intermediary representations, facilitating the conversion from
/// placeholder types (like `GenesisInput`) to internal types (like `GenesisState`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GenesisInput {
    pub version: u32,
    pub timestamp: u32,
    pub accounts: Option<Vec<AccountInput>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum AccountInput {
    BasicFungibleFaucet(BasicFungibleFaucetInputs),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BasicFungibleFaucetInputs {
    pub auth_scheme: AuthSchemeInput,
    pub token_symbol: String,
    pub decimals: u8,
    pub max_supply: u64,
    pub storage_mode: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum AuthSchemeInput {
    RpoFalcon512,
}

impl Default for GenesisInput {
    fn default() -> Self {
        Self {
            version: 1,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Current timestamp should be greater than unix epoch")
                .as_secs() as u32,
            accounts: Some(vec![AccountInput::BasicFungibleFaucet(BasicFungibleFaucetInputs {
                auth_scheme: AuthSchemeInput::RpoFalcon512,
                token_symbol: "POL".to_string(),
                decimals: 12,
                max_supply: 1_000_000,
                storage_mode: "public".to_string(),
            })]),
        }
    }
}
