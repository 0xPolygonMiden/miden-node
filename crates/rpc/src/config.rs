use std::fmt::{Display, Formatter};

use miden_node_utils::config::{
    DEFAULT_BLOCK_PRODUCER_PORT, DEFAULT_NODE_RPC_PORT, DEFAULT_STORE_PORT,
};
use serde::{Deserialize, Serialize};

// Main config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcConfig {
    pub endpoint: String,
    /// Store gRPC endpoint in the format `http://<host>[:<port>]`.
    pub store_url: String,
    /// Block producer gRPC endpoint in the format `http://<host>[:<port>]`.
    pub block_producer_url: String,
}

impl RpcConfig {
    pub fn endpoint_url(&self) -> String {
        self.endpoint.to_string()
    }
}

impl Display for RpcConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ endpoint: \"{}\", store_url: \"{}\", block_producer_url: \"{}\" }}",
            self.endpoint, self.store_url, self.block_producer_url
        ))
    }
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            endpoint: format!("0.0.0.0:{DEFAULT_NODE_RPC_PORT}"),
            store_url: format!("http://127.0.0.1:{DEFAULT_STORE_PORT}"),
            block_producer_url: format!("http://127.0.0.1:{DEFAULT_BLOCK_PRODUCER_PORT}"),
        }
    }
}
