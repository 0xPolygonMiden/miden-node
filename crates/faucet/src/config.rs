use miden_node_utils::config::Endpoint;
use serde::{Deserialize, Serialize};

pub const CONFIG_FILENAME: &str = "miden-faucet.toml";

// Faucet config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct FaucetConfig {
    /// Endpoint of the faucet
    pub endpoint: Endpoint,
    /// Location to store database files
    pub database_filepath: String,
    /// Amount of asset that should be dispered on each faucet request
    pub asset_amount: u64,
    /// Token symbol of the generated fungible asset
    pub token_symbol: String,
    /// Number of decimals of the generated fungible asset
    pub decimals: u8,
    /// Maximum supply of the generated fungible asset
    pub max_supply: u64,
}

impl FaucetConfig {
    pub fn as_url(&self) -> String {
        self.endpoint.to_string()
    }
}

// Top-level config
// ================================================================================================

/// Faucet top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct FaucetTopLevelConfig {
    pub faucet: FaucetConfig,
}
