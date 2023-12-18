mod cli;
use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use miden_node_store::{config::StoreTopLevelConfig, db::Db, server};
use miden_node_utils::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();
    let config: StoreTopLevelConfig =
        StoreTopLevelConfig::load_config(cli.config.as_deref()).extract()?;
    let db = Db::get_conn(config.store.clone()).await?;

    match cli.command {
        Command::Serve { .. } => {
            server::api::serve(config.store, db).await?;
        },
    }

    Ok(())
}
