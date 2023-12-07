mod cli;
mod errors;
use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use miden_node_store::{config::StoreConfig, db::Db, server};
use miden_node_utils::Config;

#[tokio::main]
async fn main() -> Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();
    let config: StoreConfig = StoreConfig::load_config(cli.config.as_deref()).extract()?;
    let db = Db::get_conn(config.clone()).await?;

    match cli.command {
        Command::Serve { .. } => {
            server::api::serve(config, db).await?;
        },
    }

    Ok(())
}
