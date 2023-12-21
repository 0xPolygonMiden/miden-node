use miden_node_utils::config::Endpoint;
use serde::{Deserialize, Serialize};

pub const CONFIG_FILENAME: &str = "miden-block-producer.toml";

// Main config
// ================================================================================================

/// Block producer specific configuration
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct BlockProducerConfig {
    pub endpoint: Endpoint,

    /// Store gRPC endpoint in the format `http://<host>[:<port>]`.
    pub store_url: String,
}

impl BlockProducerConfig {
    pub fn as_url(&self) -> String {
        format!("http://{}:{}", self.endpoint.host, self.endpoint.port)
    }
}

// Top-level config
// ================================================================================================

/// Block producer top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct BlockProducerTopLevelConfig {
    pub block_producer: BlockProducerConfig,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use figment::Jail;
    use miden_node_utils::config::{load_config, Endpoint};

    use super::{BlockProducerConfig, BlockProducerTopLevelConfig};
    use crate::config::CONFIG_FILENAME;

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
                load_config(PathBuf::from(CONFIG_FILENAME).as_path()).extract()?;

            assert_eq!(
                config,
                BlockProducerTopLevelConfig {
                    block_producer: BlockProducerConfig {
                        endpoint: Endpoint {
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
