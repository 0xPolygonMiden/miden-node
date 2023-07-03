pub mod cli;
pub mod config;
pub mod server;
use miden_node_utils::Config;

use clap::Parser;
use cli::{Cli, Command};
use server::api;
use config::RpcConfig;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();

    let config = RpcConfig::load_config(cli.config.as_deref()).extract()?;

    match cli.command {
        Command::Serve { .. } => {
            api::serve(config).await?;
        }
    }

    Ok(())
}
