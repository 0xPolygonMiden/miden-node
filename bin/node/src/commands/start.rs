use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use miden_node_block_producer::{config::BlockProducerConfig, server as block_producer_server};
use miden_node_rpc::{config::RpcConfig, server as rpc_server};
use miden_node_store::server::Store;
use tokio::task::JoinSet;

use crate::config::NodeConfig;

// START
// ===================================================================================================

pub async fn start_node(config: NodeConfig) -> Result<()> {
    let (block_producer, rpc, store) = config.into_parts();

    let mut join_set = JoinSet::new();

    // Start store. The store endpoint is available after loading completes.
    let store = Store::load(store).await.context("Loading store")?;
    join_set.spawn(async move { store.serve().await.context("Serving store") });

    // Wait for store to start & start block-producer
    join_set.spawn(start_block_producer(block_producer));

    // Wait for block-producer to start & start rpc
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(start_rpc(rpc));

    // block on all tasks
    while let Some(res) = join_set.join_next().await {
        // For now, if one of the components fails, crash the node
        res??;
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
