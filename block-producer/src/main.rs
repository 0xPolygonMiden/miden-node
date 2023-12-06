use anyhow::Result;
use clap::Parser;
use miden_node_block_producer::{
    cli::{Cli, Command},
    config::BlockProducerConfig,
    server,
};
use miden_node_utils::Config;

#[tokio::main]
async fn main() -> Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();
    let config: BlockProducerConfig =
        BlockProducerConfig::load_config(cli.config.as_deref()).extract()?;

    match cli.command {
        Command::Serve { .. } => {
            server::api::serve(config).await?;
        },
    }

    Ok(())
}
