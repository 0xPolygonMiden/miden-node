use std::path::PathBuf;

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand};
use commands::{
    init::init_config_files,
    start::{start_block_producer, start_node, start_rpc, start_store},
};
use miden_node_utils::config::load_config;

mod commands;
mod config;

// CONSTANTS
// ================================================================================================

const NODE_CONFIG_FILE_PATH: &str = "miden-node.toml";
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
        #[command(subcommand)]
        command: StartCommand,

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

    /// Generates default configuration files for the node
    ///
    /// This command creates two files (miden-node.toml and genesis.toml) that provide configuration
    /// details to the node. These files may be modified to change the node behavior.
    Init {
        #[arg(short, long, default_value = NODE_CONFIG_FILE_PATH)]
        config_path: String,

        #[arg(short, long, default_value = DEFAULT_GENESIS_INPUTS_PATH)]
        genesis_path: String,
    },
}

#[derive(Subcommand)]
pub enum StartCommand {
    Node,
    BlockProducer,
    Rpc,
    Store,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();

    match &cli.command {
        Command::Start { command, config } => match command {
            StartCommand::Node => {
                let config = load_config(config).context("Loading configuration file")?;
                start_node(config).await
            },
            StartCommand::BlockProducer => {
                let config = load_config(config).context("Loading configuration file")?;
                start_block_producer(config).await
            },
            StartCommand::Rpc => {
                let config = load_config(config).context("Loading configuration file")?;
                start_rpc(config).await
            },
            StartCommand::Store => {
                let config = load_config(config).context("Loading configuration file")?;
                start_store(config).await
            },
        },
        Command::MakeGenesis { output_path, force, inputs_path } => {
            commands::make_genesis(inputs_path, output_path, force)
        },
        Command::Init { config_path, genesis_path } => {
            let current_dir = std::env::current_dir()
                .map_err(|err| anyhow!("failed to open current directory: {err}"))?;

            let mut config = current_dir.clone();
            let mut genesis = current_dir.clone();

            config.push(config_path);
            genesis.push(genesis_path);

            init_config_files(config, genesis)
        },
    }
}
