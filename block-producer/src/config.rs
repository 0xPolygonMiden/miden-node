use std::fmt::{Display, Formatter};

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

    pub verify_tx_proofs: bool,
}

impl BlockProducerConfig {
    pub fn as_url(&self) -> String {
        self.endpoint.to_string()
    }
}

impl Display for BlockProducerConfig {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ endpoint: \"{}\", store_url: \"{}\" }}",
            self.endpoint, self.store_url
        ))
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
                    verify_tx_proofs = true

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
                        verify_tx_proofs: true
                    }
                }
            );

            Ok(())
        });
    }
}
