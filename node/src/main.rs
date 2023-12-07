use std::{ops::Deref, path::PathBuf};

use miden_node_store::{
    config::{self as store_config, StoreConfig},
    db::Db,
    server,
};
use miden_node_utils::Config;
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let mut join_set = JoinSet::new();

    // start store
    {
        let store_config = PathBuf::from(store_config::CONFIG_FILENAME);

        let config: StoreConfig =
            StoreConfig::load_config(Some(store_config).as_deref()).extract()?;
        let db = Db::get_conn(config.clone()).await?;

        join_set.spawn(server::api::serve(config, db));
    }

    Ok(())
}
