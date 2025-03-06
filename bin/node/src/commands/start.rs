use std::collections::HashMap;

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
    let store_id = join_set.spawn(async move { store.serve().await.context("Serving store") }).id();

    // Start block-producer. The block-producer's endpoint is available after loading completes.
    let block_producer =
        BlockProducer::init(block_producer).await.context("Loading block-producer")?;
    let block_producer_id = join_set
        .spawn(async move { block_producer.serve().await.context("Serving block-producer") })
        .id();

    // Start RPC component.
    let rpc = Rpc::init(rpc.endpoint, rpc.store_url, rpc.block_producer_url)
        .await
        .context("Loading RPC")?;
    let rpc_id = join_set.spawn(async move { rpc.serve().await.context("Serving RPC") }).id();

    // Lookup table so we can identify the failed component.
    let component_ids = HashMap::from([
        (store_id, "store"),
        (block_producer_id, "block-producer"),
        (rpc_id, "rpc"),
    ]);

    // SAFETY: The joinset is definitely not empty.
    let component_result = join_set.join_next_with_id().await.unwrap();

    // We expect components to run indefinitely, so we treat any return as fatal.
    //
    // Map all outcomes to an error, and provide component context.
    let (id, err) = match component_result {
        Ok((id, Ok(_))) => (id, Err(anyhow::anyhow!("Component completed unexpectedly"))),
        Ok((id, Err(err))) => (id, Err(err)),
        Err(join_err) => (join_err.id(), Err(join_err).context("Joining component task")),
    };
    let component = component_ids.get(&id).unwrap_or(&"unknown");

    // We could abort and gracefully shutdown the other components, but since we're crashing the
    // node there is no point.

    err.context(format!("Component {component} failed"))
}
