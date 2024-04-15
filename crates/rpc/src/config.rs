use std::fmt::{Display, Formatter};

use miden_node_utils::config::Endpoint;
use serde::{Deserialize, Serialize};

pub const CONFIG_FILENAME: &str = "miden-rpc.toml";

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

// Top-level config
// ================================================================================================

/// Rpc top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct RpcTopLevelConfig {
    pub rpc: RpcConfig,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use figment::Jail;
    use miden_node_utils::config::{load_config, Endpoint};

    use super::{RpcConfig, RpcTopLevelConfig, CONFIG_FILENAME};

    #[test]
    fn test_rpc_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                CONFIG_FILENAME,
                r#"
                    [rpc]
                    store_url = "http://store:8000"
                    block_producer_url = "http://block_producer:8001"

                    [rpc.endpoint]
                    host = "127.0.0.1"
                    port = 8080
                "#,
            )?;

            let config: RpcTopLevelConfig =
                load_config(PathBuf::from(CONFIG_FILENAME).as_path()).extract()?;

            assert_eq!(
                config,
                RpcTopLevelConfig {
                    rpc: RpcConfig {
                        endpoint: Endpoint {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        store_url: "http://store:8000".to_string(),
                        block_producer_url: "http://block_producer:8001".to_string(),
                    }
                }
            );

            Ok(())
        });
    }
}
