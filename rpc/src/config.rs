use miden_node_block_producer::config::BlockProducerConfig;
use miden_node_store::config::StoreConfig;
use miden_node_utils::config::{Config, HostPort};
use serde::{Deserialize, Serialize};

pub const HOST: &str = "localhost";
// defined as: sum(ord(c)**p for (p, c) in enumerate('miden-rpc', 1)) % 2**16
pub const PORT: u16 = 57291;
pub const CONFIG_FILENAME: &str = "miden-rpc.toml";

// Main config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct RpcConfig {
    pub host_port: HostPort,
    /// Store gRPC endpoint in the format `http://<host>[:<port>]`.
    pub store_endpoint: String,
    /// Block producer gRPC endpoint in the format `http://<host>[:<port>]`.
    pub block_producer_endpoint: String,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            host_port: HostPort {
                host: HOST.to_string(),
                port: PORT,
            },
            store_endpoint: StoreConfig::default().as_endpoint(),
            block_producer_endpoint: BlockProducerConfig::default().as_endpoint(),
        }
    }
}

impl RpcConfig {
    pub fn as_endpoint(&self) -> String {
        format!("http://{}:{}", self.host_port.host, self.host_port.port)
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
    use figment::Jail;
    use miden_node_utils::{config::HostPort, Config};

    use super::{RpcConfig, RpcTopLevelConfig, CONFIG_FILENAME};

    #[test]
    fn test_rpc_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                CONFIG_FILENAME,
                r#"
                    [rpc]
                    store_endpoint = "http://store:8000"
                    block_producer_endpoint = "http://block_producer:8001"

                    [rpc.host_port]
                    host = "127.0.0.1"
                    port = 8080
                "#,
            )?;

            let config: RpcTopLevelConfig = RpcTopLevelConfig::load_config(None).extract()?;

            assert_eq!(
                config,
                RpcTopLevelConfig {
                    rpc: RpcConfig {
                        host_port: HostPort {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        store_endpoint: "http://store:8000".to_string(),
                        block_producer_endpoint: "http://block_producer:8001".to_string(),
                    }
                }
            );

            Ok(())
        });
    }
}
