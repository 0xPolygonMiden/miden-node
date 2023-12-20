use miden_node_block_producer::config::BlockProducerConfig;
use miden_node_rpc::config::RpcConfig;
use miden_node_store::config::StoreConfig;
use miden_node_utils::config::Config;
use serde::{Deserialize, Serialize};

pub const CONFIG_FILENAME: &str = "miden-node.toml";

// Top-level config
// ================================================================================================

/// Node top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize, Default)]
pub struct NodeTopLevelConfig {
    pub block_producer: BlockProducerConfig,
    pub rpc: RpcConfig,
    pub store: StoreConfig,
}

impl Config for NodeTopLevelConfig {
    const CONFIG_FILENAME: &'static str = CONFIG_FILENAME;
}

#[cfg(test)]
mod tests {
    use figment::Jail;
    use miden_node_block_producer::config::BlockProducerConfig;
    use miden_node_rpc::config::RpcConfig;
    use miden_node_store::config::StoreConfig;
    use miden_node_utils::{config::HostPort, Config};

    use super::{NodeTopLevelConfig, CONFIG_FILENAME};

    #[test]
    fn test_node_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                CONFIG_FILENAME,
                r#"
                    [block_producer]
                    store_endpoint = "http://store:8000"

                    [block_producer.host_port]
                    host = "127.0.0.1"
                    port = 8080

                    [rpc]
                    store_endpoint = "http://store:8000"
                    block_producer_endpoint = "http://block_producer:8001"
                    host_port = { host = "127.0.0.1",  port = 8080 }

                    [store]
                    database_filepath = "local.sqlite3"
                    genesis_filepath = "genesis.bin"

                    [store.endpoint]
                    host = "127.0.0.1"
                    port = 8080
                "#,
            )?;

            let config: NodeTopLevelConfig = NodeTopLevelConfig::load_config(None).extract()?;

            assert_eq!(
                config,
                NodeTopLevelConfig {
                    block_producer: BlockProducerConfig {
                        host_port: HostPort {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        store_endpoint: "http://store:8000".to_string(),
                    },
                    rpc: RpcConfig {
                        host_port: HostPort {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        store_endpoint: "http://store:8000".to_string(),
                        block_producer_endpoint: "http://block_producer:8001".to_string(),
                    },
                    store: StoreConfig {
                        endpoint: HostPort {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        database_filepath: "local.sqlite3".into(),
                        genesis_filepath: "genesis.bin".into()
                    },
                }
            );

            Ok(())
        });
    }

    #[test]
    fn test_node_config_env() {
        Jail::expect_with(|jail| {
            // Block producer
            // ------------------------------------------------------------------------------------
            jail.set_env("MIDEN__BLOCK_PRODUCER__STORE_ENDPOINT", "http://store:8000");
            jail.set_env("MIDEN__BLOCK_PRODUCER__HOST_PORT__HOST", "127.0.0.1");
            jail.set_env("MIDEN__BLOCK_PRODUCER__HOST_PORT__PORT", 8080);

            // Rpc
            // ------------------------------------------------------------------------------------
            jail.set_env("MIDEN__RPC__STORE_ENDPOINT", "http://store:8000");
            jail.set_env("MIDEN__RPC__BLOCK_PRODUCER_ENDPOINT", "http://block_producer:8001");
            jail.set_env("MIDEN__RPC__HOST_PORT__HOST", "127.0.0.1");
            jail.set_env("MIDEN__RPC__HOST_PORT__PORT", 8080);

            // Store
            // ------------------------------------------------------------------------------------
            jail.set_env("MIDEN__STORE__DATABASE_FILEPATH", "local.sqlite3");
            jail.set_env("MIDEN__STORE__GENESIS_FILEPATH", "genesis.bin");
            jail.set_env("MIDEN__STORE__ENDPOINT__HOST", "127.0.0.1");
            jail.set_env("MIDEN__STORE__ENDPOINT__PORT", 8080);

            let config: NodeTopLevelConfig = NodeTopLevelConfig::load_config(None).extract()?;

            assert_eq!(
                config,
                NodeTopLevelConfig {
                    block_producer: BlockProducerConfig {
                        host_port: HostPort {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        store_endpoint: "http://store:8000".to_string(),
                    },
                    rpc: RpcConfig {
                        host_port: HostPort {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        store_endpoint: "http://store:8000".to_string(),
                        block_producer_endpoint: "http://block_producer:8001".to_string(),
                    },
                    store: StoreConfig {
                        endpoint: HostPort {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        database_filepath: "local.sqlite3".into(),
                        genesis_filepath: "genesis.bin".into()
                    },
                }
            );

            Ok(())
        });
    }
}
