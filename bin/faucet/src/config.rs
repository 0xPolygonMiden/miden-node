use std::{
    fmt::{Display, Formatter},
    path::PathBuf,
};

use miden_node_utils::config::{Endpoint, DEFAULT_FAUCET_SERVER_PORT, DEFAULT_NODE_RPC_PORT};
use serde::{Deserialize, Serialize};

// Faucet config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct FaucetConfig {
    /// Endpoint of the faucet
    pub endpoint: Endpoint,
    /// Node RPC gRPC endpoint in the format `http://<host>[:<port>]`.
    pub node_url: String,
    /// Timeout for RPC requests in milliseconds
    pub timeout_ms: u64,
    /// Location to store database files
    pub database_filepath: PathBuf,
    /// Possible options on the amount of asset that should be dispersed on each faucet request
    pub asset_amount_options: Vec<u64>,
    /// Token symbol of the generated fungible asset
    pub token_symbol: String,
    /// Number of decimals of the generated fungible asset
    pub decimals: u8,
    /// Maximum supply of the generated fungible asset
    pub max_supply: u64,
}

impl FaucetConfig {
    pub fn endpoint_url(&self) -> String {
        self.endpoint.to_string()
    }
}

impl Display for FaucetConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ endpoint: \"{}\",  database_filepath: {:?}, asset_amount_options: {:?}, token_symbol: {}, decimals: {}, max_supply: {} }}",
            self.endpoint, self.database_filepath, self.asset_amount_options, self.token_symbol, self.decimals, self.max_supply
        ))
    }
}

impl Default for FaucetConfig {
    fn default() -> Self {
        Self {
            endpoint: Endpoint::localhost(DEFAULT_FAUCET_SERVER_PORT),
            node_url: Endpoint::localhost(DEFAULT_NODE_RPC_PORT).to_string(),
            timeout_ms: 10000,
            database_filepath: PathBuf::from("store.sqlite3"),
            asset_amount_options: vec![100, 500, 1000],
            token_symbol: "POL".to_string(),
            decimals: 8,
            max_supply: 1000000,
        }
    }
}
