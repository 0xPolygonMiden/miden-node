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
    BasicWallet(BasicWalletInputs),
    BasicFungibleFaucet(BasicFungibleFaucetInputs),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BasicWalletInputs {
    pub init_seed: String,
    pub auth_scheme: AuthSchemeInput,
    pub auth_seed: String,
    pub storage_mode: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BasicFungibleFaucetInputs {
    pub init_seed: String,
    pub auth_scheme: AuthSchemeInput,
    pub auth_seed: String,
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
            accounts: Some(vec![
                AccountInput::BasicWallet(BasicWalletInputs {
                    init_seed: "0xa123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
                    auth_scheme: AuthSchemeInput::RpoFalcon512,
                    auth_seed: "0xb123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
                    storage_mode: "off-chain".to_string(),
                }),
                AccountInput::BasicFungibleFaucet(BasicFungibleFaucetInputs {
                    init_seed: "0xc123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
                    auth_scheme: AuthSchemeInput::RpoFalcon512,
                    auth_seed: "0xd123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
                    token_symbol: "POL".to_string(),
                    decimals: 12,
                    max_supply: 1000000,
                    storage_mode: "on-chain".to_string(),
                }),
            ]),
        }
    }
}
