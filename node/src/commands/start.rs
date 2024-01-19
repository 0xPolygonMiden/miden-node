use miden_node_block_producer::config::BlockProducerConfig;
use miden_node_rpc::config::RpcConfig;
use miden_node_store::config::StoreConfig;
use miden_node_utils::control_plane::{ControlPlane, ControlPlaneConfig};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

use anyhow::{anyhow, Result};
use miden_node_block_producer::server as block_producer_server;
use miden_node_rpc::server as rpc_server;
use miden_node_store::{db::Db, server as store_server};
use miden_node_utils::config::load_config;
use tokio::task::JoinSet;

// Top-level config
// ================================================================================================

/// Node top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct StartCommandConfig {
    pub block_producer: BlockProducerConfig,
    pub rpc: RpcConfig,
    pub store: StoreConfig,
    pub control_plane: ControlPlaneConfig,
}

// START
// ===================================================================================================

pub async fn start_node(config_filepath: &Path) -> Result<()> {
    let config: StartCommandConfig = load_config(config_filepath).extract().map_err(|err| {
        anyhow!("failed to load config file `{}`: {err}", config_filepath.display())
    })?;

    let mut control_plane = ControlPlane::new();
    let mut join_set = JoinSet::new();

    let db = Db::setup(config.store.clone()).await?;
    let shutdown = control_plane.shutdown_waiter()?;
    let store_server = store_server::create_server(config.store, db, shutdown).await?;
    join_set.spawn(store_server);

    // wait for store before starting block producer
    tokio::time::sleep(Duration::from_secs(1)).await;
    let block_server = block_producer_server::serve(config.block_producer);
    join_set.spawn(block_server);

    // wait for block producer before starting rpc
    tokio::time::sleep(Duration::from_secs(1)).await;
    let shutdown = control_plane.shutdown_waiter()?;
    let rpc_server = rpc_server::create_server(config.rpc, shutdown).await?;
    join_set.spawn(rpc_server);

    // block on all tasks
    while let Some(res) = join_set.join_next().await {
        // For now, if one of the components fails, crash the node
        res.unwrap().unwrap();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use figment::Jail;
    use miden_node_block_producer::config::BlockProducerConfig;
    use miden_node_rpc::config::RpcConfig;
    use miden_node_store::config::StoreConfig;
    use miden_node_utils::{
        config::{load_config, Endpoint},
        control_plane::ControlPlaneConfig,
    };

    use super::StartCommandConfig;
    use crate::NODE_CONFIG_FILE_PATH;

    #[test]
    fn test_node_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                NODE_CONFIG_FILE_PATH,
                r#"
                    [block_producer]
                    store_url = "http://store:82"

                    [block_producer.endpoint]
                    host = "0.0.0.0"
                    port = 81

                    [rpc]
                    store_url = "http://store:82"
                    block_producer_url = "http://block_producer:81"
                    endpoint = { host = "0.0.0.0",  port = 80 }

                    [store]
                    database_filepath = "local.sqlite3"
                    genesis_filepath = "genesis.dat"

                    [store.endpoint]
                    host = "0.0.0.0"
                    port = 82

                    [control_plane.endpoint]
                    host = "127.0.0.1"
                    port = 8080
                "#,
            )?;

            let config: StartCommandConfig =
                load_config(PathBuf::from(NODE_CONFIG_FILE_PATH).as_path()).extract()?;

            assert_eq!(
                config,
                StartCommandConfig {
                    block_producer: BlockProducerConfig {
                        endpoint: Endpoint {
                            host: "0.0.0.0".to_string(),
                            port: 81,
                        },
                        store_url: "http://store:82".to_string(),
                    },
                    rpc: RpcConfig {
                        endpoint: Endpoint {
                            host: "0.0.0.0".to_string(),
                            port: 80,
                        },
                        store_url: "http://store:82".to_string(),
                        block_producer_url: "http://block_producer:81".to_string(),
                    },
                    store: StoreConfig {
                        endpoint: Endpoint {
                            host: "0.0.0.0".to_string(),
                            port: 82,
                        },
                        database_filepath: "local.sqlite3".into(),
                        genesis_filepath: "genesis.dat".into()
                    },
                    control_plane: ControlPlaneConfig {
                        endpoint: Endpoint {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                    }
                }
            );

            Ok(())
        });
    }
}
