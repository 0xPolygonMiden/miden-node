use std::fmt::{Display, Formatter};

use miden_node_utils::config::Endpoint;
use serde::{Deserialize, Serialize};

// Main config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct RpcConfig {
    pub endpoint: Endpoint,
    /// Store gRPC endpoint in the format `http://<host>[:<port>]`.
    pub store_url: String,
    /// Block producer gRPC endpoint in the format `http://<host>[:<port>]`.
    pub block_producer_url: String,
}

impl RpcConfig {
    pub fn as_url(&self) -> String {
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
