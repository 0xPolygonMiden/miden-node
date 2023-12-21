use miden_node_block_producer::config::BlockProducerConfig;
use miden_node_rpc::config::RpcConfig;
use miden_node_store::config::StoreConfig;
use serde::{Deserialize, Serialize};

pub const CONFIG_FILENAME: &str = "miden.toml";

// Top-level config
// ================================================================================================

/// Node top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct NodeTopLevelConfig {
    pub block_producer: BlockProducerConfig,
    pub rpc: RpcConfig,
    pub store: StoreConfig,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use figment::Jail;
    use miden_node_block_producer::config::BlockProducerConfig;
    use miden_node_rpc::config::RpcConfig;
    use miden_node_store::config::StoreConfig;
    use miden_node_utils::config::{load_config, Endpoint};

    use super::{NodeTopLevelConfig, CONFIG_FILENAME};

    #[test]
    fn test_node_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                CONFIG_FILENAME,
                r#"
                    [block_producer]
                    store_url = "http://store:8000"

                    [block_producer.endpoint]
                    host = "127.0.0.1"
                    port = 8080

                    [rpc]
                    store_url = "http://store:8000"
                    block_producer_url = "http://block_producer:8001"
                    endpoint = { host = "127.0.0.1",  port = 8080 }

                    [store]
                    database_filepath = "local.sqlite3"
                    genesis_filepath = "genesis.dat"

                    [store.endpoint]
                    host = "127.0.0.1"
                    port = 8080
                "#,
            )?;

            let config: NodeTopLevelConfig =
                load_config(PathBuf::from(CONFIG_FILENAME).as_path()).extract()?;

            assert_eq!(
                config,
                NodeTopLevelConfig {
                    block_producer: BlockProducerConfig {
                        endpoint: Endpoint {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        store_url: "http://store:8000".to_string(),
                    },
                    rpc: RpcConfig {
                        endpoint: Endpoint {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        store_url: "http://store:8000".to_string(),
                        block_producer_url: "http://block_producer:8001".to_string(),
                    },
                    store: StoreConfig {
                        endpoint: Endpoint {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        database_filepath: "local.sqlite3".into(),
                        genesis_filepath: "genesis.dat".into()
                    },
                }
            );

            Ok(())
        });
    }
}
