pub mod cli;
use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use miden_node_rpc::{config::RpcTopLevelConfig, server};
use miden_node_utils::config::load_config;

#[tokio::main]
async fn main() -> Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();

    let config: RpcTopLevelConfig = load_config(cli.config.as_path()).extract()?;

    match cli.command {
        Command::Serve => {
            server::serve(config.rpc).await?;
        },
    }

    Ok(())
}
