mod cli;
use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use miden_node_store::{config::StoreTopLevelConfig, db::Db, server};
use miden_node_utils::config::load_config;

#[tokio::main]
async fn main() -> Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();
    let config: StoreTopLevelConfig = load_config(cli.config.as_path()).extract()?;
    let db = Db::setup(config.store.clone()).await?;

    match cli.command {
        Command::Serve { .. } => {
            server::serve(config.store, db).await?;
        },
    }

    Ok(())
}
