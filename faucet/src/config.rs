use std::fmt::{Display, Formatter};

use miden_node_utils::config::Endpoint;
use serde::{Deserialize, Serialize};

pub const CONFIG_FILENAME: &str = "miden-faucet.toml";

// Main config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct FaucetConfig {
    pub endpoint: Endpoint,
    /// rpc gRPC endpoint in the format `http://<host>[:<port>]`.
    pub rpc_url: String,
    /// Location to store database files
    pub database_filepath: String,
}

impl FaucetConfig {
    pub fn as_url(&self) -> String {
        self.endpoint.to_string()
    }
}

impl Display for FaucetConfig {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ endpoint: \"{}\", store_url: \"{}\", block_producer_url: \"{}\" }}",
            self.endpoint, self.database_filepath, self.rpc_url
        ))
    }
}

// Top-level config
// ================================================================================================

/// Faucet top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct FaucetTopLevelConfig {
    pub faucet: FaucetConfig,
}
