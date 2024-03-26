use std::path::Path;

use anyhow::Result;

use config::RpcTopLevelConfig;
use miden_node_utils::config::load_config;

pub mod config;
pub mod server;

// CONSTANTS
// =================================================================================================
pub const COMPONENT: &str = "miden-rpc";

// MAIN FUNCTION
// =================================================================================================

pub async fn start_rpc(config_filepath: &Path) -> Result<()> {
    // miden_node_utils::logging::setup_logging()?;

    let config: RpcTopLevelConfig = load_config(config_filepath).extract()?;

    server::serve(config.rpc).await?;

    Ok(())
}
