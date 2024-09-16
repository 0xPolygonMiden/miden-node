use anyhow::{Context, Result};
use miden_node_block_producer::server::BlockProducer;
use miden_node_rpc::server::Rpc;
use miden_node_store::server::Store;
use tokio::task::JoinSet;

use crate::config::NodeConfig;

// START
// ===================================================================================================

pub async fn start_node(config: NodeConfig) -> Result<()> {
    let (block_producer, rpc, store) = config.into_parts();

    let mut join_set = JoinSet::new();

    // Start store. The store endpoint is available after loading completes.
    let store = Store::init(store).await.context("Loading store")?;
    join_set.spawn(async move { store.serve().await.context("Serving store") });

    // Start block-producer. The block-producer's endpoint is available after loading completes.
    let block_producer =
        BlockProducer::init(block_producer).await.context("Loading block-producer")?;
    join_set.spawn(async move { block_producer.serve().await.context("Serving block-producer") });

    // Start RPC component.
    let rpc = Rpc::init(rpc).await.context("Loading RPC")?;
    join_set.spawn(async move { rpc.serve().await.context("Serving RPC") });

    // block on all tasks
    while let Some(res) = join_set.join_next().await {
        // For now, if one of the components fails, crash the node
        res??;
    }

    Ok(())
}
