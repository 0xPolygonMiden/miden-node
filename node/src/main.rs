use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod commands;

const DEFAULT_GENESIS_DAT_FILE_PATH: &str = "genesis.dat";

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the node
    Start {
        #[arg(short, long, value_name = "FILE", default_value = commands::start::CONFIG_FILENAME)]
        config: PathBuf,
    },

    /// Generate genesis file
    MakeGenesis {
        #[arg(short, long, value_name = "FILE", default_value = commands::start::CONFIG_FILENAME)]
        config: PathBuf,

        #[arg(short, long, default_value = DEFAULT_GENESIS_DAT_FILE_PATH)]
        output_path: PathBuf,

        /// Generate the output file even if a file already exists
        #[arg(short, long)]
        force: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();

    match &cli.command {
        Command::Start { config } => commands::start::start(config).await,
        Command::MakeGenesis {
            output_path,
            force,
            config,
        } => commands::genesis::make_genesis(output_path, force, config),
    }
}
