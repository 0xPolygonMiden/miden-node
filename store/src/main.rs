mod cli;
mod config;
mod db;
mod migrations;
mod server;
mod types;
use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use config::StoreConfig;
use miden_node_utils::Config;
use db::Db;

#[tokio::main]
async fn main() -> Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();
    let config: StoreConfig = StoreConfig::load_config(cli.config.as_deref()).extract()?;
    let db = Db::get_conn(config.clone()).await?;

    match cli.command {
        Command::Serve { .. } => {
            server::api::serve(config, db).await?;
        }
    }

    Ok(())
}
