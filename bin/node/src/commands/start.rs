use std::time::Duration;

use anyhow::Result;
use miden_node_block_producer::{config::BlockProducerConfig, server as block_producer_server};
use miden_node_rpc::{config::RpcConfig, server as rpc_server};
use miden_node_store::{config::StoreConfig, db::Db, server as store_server};
use tokio::task::JoinSet;

use crate::StartCommandConfig;

// START
// ===================================================================================================

pub async fn start_node(config: StartCommandConfig) -> Result<()> {
    let mut join_set = JoinSet::new();
    let db = Db::setup(config.store.clone()).await?;
    join_set.spawn(store_server::serve(config.store, db));

    // wait for store before starting block producer
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(block_producer_server::serve(config.block_producer));

    // wait for block producer before starting rpc
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(rpc_server::serve(config.rpc));

    // block on all tasks
    while let Some(res) = join_set.join_next().await {
        // For now, if one of the components fails, crash the node
        res.unwrap().unwrap();
    }

    Ok(())
}

pub async fn start_block_producer(config: BlockProducerConfig) -> Result<()> {
    block_producer_server::serve(config).await?;

    Ok(())
}

pub async fn start_rpc(config: RpcConfig) -> Result<()> {
    rpc_server::serve(config).await?;

    Ok(())
}

pub async fn start_store(config: StoreConfig) -> Result<()> {
    let db = Db::setup(config.clone()).await?;

    store_server::serve(config, db).await?;

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
                    endpoint = { host = "127.0.0.1",  port = 8080 }
                    store_url = "http://store:8000"
                    verify_tx_proofs = true

                    [rpc]
                    endpoint = { host = "127.0.0.1",  port = 8080 }
                    store_url = "http://store:8000"
                    block_producer_url = "http://block_producer:8001"

                    [store]
                    endpoint = { host = "127.0.0.1",  port = 8080 }
                    database_filepath = "local.sqlite3"
                    genesis_filepath = "genesis.dat"
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
