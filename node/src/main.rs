use std::{path::PathBuf, time::Duration};

use miden_node_rpc::config as rpc_config;
use miden_node_rpc::config::RpcConfig;
use miden_node_rpc::server as rpc_server;
use miden_node_store::{
    config::{self as store_config, StoreConfig},
    db::Db,
    server as store_server,
};
use miden_node_utils::Config;
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let mut join_set = JoinSet::new();

    // start store
    {
        let config: StoreConfig = {
            let config_path = PathBuf::from(store_config::CONFIG_FILENAME);

            StoreConfig::load_config(Some(config_path).as_deref()).extract()?
        };

        let db = Db::get_conn(config.clone()).await?;

        join_set.spawn(store_server::api::serve(config, db));
    }

    // wait for store to be started
    tokio::time::sleep(Duration::from_secs(1)).await;

    // start rpc
    {
        let config: RpcConfig = {
            let config_path = PathBuf::from(rpc_config::CONFIG_FILENAME);

            RpcConfig::load_config(Some(config_path).as_deref()).extract()?
        };

        join_set.spawn(rpc_server::api::serve(config));
    }

    Ok(())
}
