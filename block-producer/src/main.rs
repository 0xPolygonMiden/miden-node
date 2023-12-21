use anyhow::Result;
use clap::Parser;
use miden_node_block_producer::{
    cli::{Cli, Command},
    config::BlockProducerTopLevelConfig,
    server,
};
use miden_node_utils::config::load_config;

#[tokio::main]
async fn main() -> Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();
    let config: BlockProducerTopLevelConfig = load_config(cli.config.as_path()).extract()?;

    match cli.command {
        Command::Serve { .. } => {
            server::api::serve(config.block_producer).await?;
        },
    }

    Ok(())
}
