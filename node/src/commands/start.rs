use std::{path::Path, time::Duration};

use anyhow::{anyhow, Result};
use miden_node_block_producer::{config::BlockProducerConfig, server as block_producer_server};
use miden_node_rpc::{config::RpcConfig, server as rpc_server};
use miden_node_store::{config::StoreConfig, db::Db, server as store_server};
use miden_node_utils::config::load_config;
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

// use crate::faucet;

// Top-level config
// ================================================================================================

/// Node top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct StartCommandConfig {
    pub block_producer: BlockProducerConfig,
    pub rpc: RpcConfig,
    pub store: StoreConfig,
}

// START
// ===================================================================================================

pub async fn start_node(
    config_filepath: &Path,
    with_faucet: &bool,
) -> Result<()> {
    let config: StartCommandConfig = load_config(config_filepath).extract().map_err(|err| {
        anyhow!("failed to load config file `{}`: {err}", config_filepath.display())
    })?;

    let mut join_set = JoinSet::new();
    let db = Db::setup(config.store.clone()).await?;
    join_set.spawn(store_server::serve(config.store, db));

    // wait for store before starting block producer
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(block_producer_server::serve(config.block_producer));

    // wait for block producer before starting rpc
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(rpc_server::serve(config.rpc));

    // if with_faucet {
    //     // start miden-faucet
    //     tokio::time::sleep(Duration::from_secs(1)).await;
    //     join_set.spawn(faucet::serve());
    // }

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
    use miden_node_utils::config::{load_config, Endpoint};

    use super::StartCommandConfig;
    use crate::NODE_CONFIG_FILE_PATH;

    #[test]
    fn test_node_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                NODE_CONFIG_FILE_PATH,
                r#"
                    [block_producer]
                    store_url = "http://store:8000"
                    verify_tx_proofs = true

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

            let config: StartCommandConfig =
                load_config(PathBuf::from(NODE_CONFIG_FILE_PATH).as_path()).extract()?;

            assert_eq!(
                config,
                StartCommandConfig {
                    block_producer: BlockProducerConfig {
                        endpoint: Endpoint {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        store_url: "http://store:8000".to_string(),
                        verify_tx_proofs: true
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
