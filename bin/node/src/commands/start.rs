use std::time::Duration;

use anyhow::Result;
use miden_node_block_producer::{config::BlockProducerConfig, server as block_producer_server};
use miden_node_faucet::{config::FaucetConfig, server as faucet_server, utils::build_faucet_state};
use miden_node_rpc::{config::RpcConfig, server as rpc_server};
use miden_node_store::{config::StoreConfig, db::Db, server as store_server};
use tokio::task::JoinSet;

use crate::config::NodeConfig;

// START
// ===================================================================================================

pub async fn start_node(config: NodeConfig) -> Result<()> {
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

pub async fn start_faucet(config: FaucetConfig) -> Result<()> {
    let faucet_state = build_faucet_state(config.clone()).await?;

    faucet_server::serve(config, faucet_state).await?;

    Ok(())
}
