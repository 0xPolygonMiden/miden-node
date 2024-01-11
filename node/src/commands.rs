use std::{path::Path, time::Duration};

use anyhow::{anyhow, Result};
use miden_node_block_producer::server as block_producer_server;
use miden_node_rpc::server as rpc_server;
use miden_node_store::{db::Db, server as store_server};
use miden_node_utils::config::load_config;
use tokio::task::JoinSet;

use crate::config::NodeTopLevelConfig;

// START
// ===================================================================================================

pub async fn start(config_filepath: &Path) -> Result<()> {
    let config: NodeTopLevelConfig = load_config(config_filepath).extract().map_err(|err| {
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

    // block on all tasks
    while let Some(res) = join_set.join_next().await {
        // For now, if one of the components fails, crash the node
        res.unwrap().unwrap();
    }

    Ok(())
}
