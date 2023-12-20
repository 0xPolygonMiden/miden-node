use miden_node_store::config::StoreConfig;
use miden_node_utils::config::{Config, HostPort};
use serde::{Deserialize, Serialize};

pub const HOST: &str = "localhost";
// defined as: sum(ord(c)**p for (p, c) in enumerate('miden-block-producer', 1)) % 2**16
pub const PORT: u16 = 48046;
pub const CONFIG_FILENAME: &str = "miden-block-producer.toml";

// Main config
// ================================================================================================

/// Block producer specific configuration
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct BlockProducerConfig {
    pub endpoint: HostPort,

    /// Store gRPC endpoint in the format `http://<host>[:<port>]`.
    pub store_url: String,
}

impl Default for BlockProducerConfig {
    fn default() -> Self {
        Self {
            endpoint: HostPort {
                host: HOST.to_string(),
                port: PORT,
            },
            store_url: StoreConfig::default().as_endpoint(),
        }
    }
}

impl BlockProducerConfig {
    pub fn as_endpoint(&self) -> String {
        format!("http://{}:{}", self.endpoint.host, self.endpoint.port)
    }
}

// Top-level config
// ================================================================================================

/// Block producer top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize, Default)]
pub struct BlockProducerTopLevelConfig {
    pub block_producer: BlockProducerConfig,
}

impl Config for BlockProducerTopLevelConfig {
    const CONFIG_FILENAME: &'static str = CONFIG_FILENAME;
}

#[cfg(test)]
mod tests {
    use figment::Jail;
    use miden_node_utils::{config::HostPort, Config};

    use super::{BlockProducerConfig, BlockProducerTopLevelConfig, CONFIG_FILENAME};

    #[test]
    fn test_block_producer_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                CONFIG_FILENAME,
                r#"
                    [block_producer]
                    store_url = "http://store:8000"

                    [block_producer.endpoint]
                    host = "127.0.0.1"
                    port = 8080
                "#,
            )?;

            let config: BlockProducerTopLevelConfig =
                BlockProducerTopLevelConfig::load_config(None).extract()?;

            assert_eq!(
                config,
                BlockProducerTopLevelConfig {
                    block_producer: BlockProducerConfig {
                        endpoint: HostPort {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        store_url: "http://store:8000".to_string(),
                    }
                }
            );

            Ok(())
        });
    }
}
