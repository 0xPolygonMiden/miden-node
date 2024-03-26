use std::path::Path;

use anyhow::Result;

use db::Db;
use config::StoreTopLevelConfig;
use miden_node_utils::config::load_config;

mod nullifier_tree;
pub mod config;
pub mod db;
pub mod errors;
pub mod genesis;
pub mod server;
pub mod state;
pub mod types;

// CONSTANTS
// =================================================================================================
pub const COMPONENT: &str = "miden-store";

// MAIN FUNCTION
// =================================================================================================

pub async fn start_store(config_filepath: &Path) -> Result<()> {
    let config: StoreTopLevelConfig = load_config(config_filepath).extract()?;
    let db = Db::setup(config.store.clone()).await?;

    server::serve(config.store, db).await?;

    Ok(())
}
