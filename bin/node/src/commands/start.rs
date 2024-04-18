use std::time::Duration;

use anyhow::{anyhow, Context, Result};
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

    // Start store
    join_set.spawn(start_store(config.store.context("Missing store configuration.")?));

    // Wait for store to start & start block-producer
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(start_block_producer(
        config.block_producer.context("Missing block-producer configuration.")?,
    ));

    // Wait for block-producer to start & start rpc
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(start_rpc(config.rpc.context("Missing rpc configuration.")?));

    // block on all tasks
    while let Some(res) = join_set.join_next().await {
        // For now, if one of the components fails, crash the node
        res.unwrap().unwrap();
    }

    Ok(())
}

pub async fn start_block_producer(config: BlockProducerConfig) -> Result<()> {
    block_producer_server::serve(config)
        .await
        .map_err(|err| anyhow!("Failed to serve block-producer: {}", err))?;

    Ok(())
}

pub async fn start_rpc(config: RpcConfig) -> Result<()> {
    rpc_server::serve(config)
        .await
        .map_err(|err| anyhow!("Failed to serve rpc: {}", err))?;

    Ok(())
}

pub async fn start_store(config: StoreConfig) -> Result<()> {
    let db = Db::setup(config.clone())
        .await
        .map_err(|err| anyhow!("Failed to setup database: {}", err))?;

    store_server::serve(config, db)
        .await
        .map_err(|err| anyhow!("Failed to serve store: {}", err))?;

    Ok(())
}

pub async fn start_faucet(config: FaucetConfig) -> Result<()> {
    let faucet_state = build_faucet_state(config.clone())
        .await
        .map_err(|err| anyhow!("Failed to build faucet: {}", err))?;

    faucet_server::serve(config, faucet_state).await?;

    Ok(())
}
