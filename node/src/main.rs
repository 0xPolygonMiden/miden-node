use std::{path::Path, time::Duration};

use config::{NodeTopLevelConfig, CONFIG_FILENAME};
use miden_node_block_producer::server as block_producer_server;
use miden_node_rpc::server as rpc_server;
use miden_node_store::{db::Db, server as store_server};
use miden_node_utils::Config;
use tokio::task::JoinSet;

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let config: NodeTopLevelConfig =
        NodeTopLevelConfig::load_config(Some(Path::new(CONFIG_FILENAME))).extract()?;

    let mut join_set = JoinSet::new();
    let db = Db::get_conn(config.store.clone()).await?;
    join_set.spawn(store_server::api::serve(config.store, db));

    // wait for store before starting block producer
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(block_producer_server::api::serve(config.block_producer));

    // wait for blockproducer before starting rpc
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(rpc_server::api::serve(config.rpc));

    // block on all tasks
    while let Some(_res) = join_set.join_next().await {
        // do nothing
    }

    Ok(())
}
