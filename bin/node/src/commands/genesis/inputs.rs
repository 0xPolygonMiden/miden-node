use serde::Deserialize;

// INPUT HELPER STRUCTS
// ================================================================================================

/// Input types are helper structures designed for parsing and deserializing genesis input files.
/// They serve as intermediary representations, facilitating the conversion from
/// placeholder types (like `GenesisInput`) to internal types (like `GenesisState`).
#[derive(Debug, Clone, Deserialize)]
pub struct GenesisInput {
    pub version: u32,
    pub timestamp: u32,
    pub accounts: Vec<AccountInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum AccountInput {
    BasicWallet(BasicWalletInputs),
    BasicFungibleFaucet(BasicFungibleFaucetInputs),
}

#[derive(Debug, Clone, Deserialize)]
pub struct BasicWalletInputs {
    pub init_seed: String,
    pub auth_scheme: AuthSchemeInput,
    pub auth_seed: String,
    #[serde(default = "_default_true")]
    pub on_chain: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BasicFungibleFaucetInputs {
    pub init_seed: String,
    pub auth_scheme: AuthSchemeInput,
    pub auth_seed: String,
    pub token_symbol: String,
    pub decimals: u8,
    pub max_supply: u64,
    #[serde(default = "_default_true")]
    pub on_chain: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum AuthSchemeInput {
    RpoFalcon512,
}

const fn _default_true() -> bool {
    true
}
