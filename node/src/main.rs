use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod commands;

// CONSTANTS
// ================================================================================================

const NODE_CONFIG_FILE_PATH: &str = "node.toml";

const DEFAULT_GENESIS_FILE_PATH: &str = "genesis.dat";
const DEFAULT_GENESIS_INPUTS_PATH: &str = "genesis.toml";

// COMMANDS
// ================================================================================================

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
        #[arg(short, long, value_name = "FILE", default_value = NODE_CONFIG_FILE_PATH)]
        config: PathBuf,
    },

    /// Generates a genesis file and associated account files based on a specified genesis input
    ///
    /// This command creates a new genesis file and associated account files at the specified output
    /// paths. It checks for the existence of the output file, and if it already exists, an error is
    /// thrown unless the `force` flag is set to overwrite it.
    MakeGenesis {
        /// Read genesis file inputs from this location
        #[arg(short, long, value_name = "FILE", default_value = DEFAULT_GENESIS_INPUTS_PATH)]
        inputs_path: PathBuf,

        /// Write the genesis file to this location
        #[arg(short, long, value_name = "FILE", default_value = DEFAULT_GENESIS_FILE_PATH)]
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
        Command::Start { config } => commands::start_node(config).await,
        Command::MakeGenesis {
            output_path,
            force,
            inputs_path,
        } => commands::make_genesis(inputs_path, output_path, force),
    }
}
