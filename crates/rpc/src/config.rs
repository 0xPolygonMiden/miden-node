use std::fmt::{Display, Formatter};

use miden_node_utils::config::{
    DEFAULT_BLOCK_PRODUCER_PORT, DEFAULT_NODE_RPC_PORT, DEFAULT_STORE_PORT,
};
use serde::{Deserialize, Serialize};
use url::Url;

// Main config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcConfig {
    pub endpoint: Url,
    /// Store gRPC endpoint in the format `http://<host>[:<port>]`.
    pub store_url: Url,
    /// Block producer gRPC endpoint in the format `http://<host>[:<port>]`.
    pub block_producer_url: Url,
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
            endpoint: Url::parse(format!("0.0.0.0:{DEFAULT_NODE_RPC_PORT}").as_str()).unwrap(),
            store_url: Url::parse(format!("http://127.0.0.1:{DEFAULT_STORE_PORT}").as_str())
                .unwrap(),
            block_producer_url: Url::parse(
                format!("http://127.0.0.1:{DEFAULT_BLOCK_PRODUCER_PORT}").as_str(),
            )
            .unwrap(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rpc_config() {
        let config = RpcConfig::default();
        assert_eq!(config.endpoint.path(), "");
        assert_eq!(config.endpoint.host().unwrap().to_string(), "0.0.0.0");
        assert_eq!(config.endpoint.scheme(), "http");
    }
}
