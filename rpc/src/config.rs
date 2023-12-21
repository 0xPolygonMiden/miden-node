use miden_node_block_producer::config::BlockProducerConfig;
use miden_node_store::config::StoreConfig;
use miden_node_utils::config::{Config, Endpoint};
use serde::{Deserialize, Serialize};

pub const HOST: &str = "localhost";
// defined as: sum(ord(c)**p for (p, c) in enumerate('miden-rpc', 1)) % 2**16
pub const PORT: u16 = 57291;
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

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            endpoint: Endpoint {
                host: HOST.to_string(),
                port: PORT,
            },
            store_url: StoreConfig::default().as_url(),
            block_producer_url: BlockProducerConfig::default().as_url(),
        }
    }
}

impl RpcConfig {
    pub fn as_url(&self) -> String {
        format!("http://{}:{}", self.endpoint.host, self.endpoint.port)
    }
}

// Top-level config
// ================================================================================================

/// Rpc top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize, Default)]
pub struct RpcTopLevelConfig {
    pub rpc: RpcConfig,
}

impl Config for RpcTopLevelConfig {
    const CONFIG_FILENAME: &'static str = CONFIG_FILENAME;
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use figment::Jail;
    use miden_node_utils::{config::Endpoint, Config};

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
                RpcTopLevelConfig::load_config(Some(PathBuf::from(CONFIG_FILENAME).as_path()))
                    .extract()?;

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
