use std::path::PathBuf;

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use commands::start_node;
use miden_node_block_producer::start_block_producer;
use miden_node_faucet::start_faucet;
use miden_node_rpc::start_rpc;
use miden_node_store::start_store;

mod commands;

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
}

#[derive(Subcommand)]
pub enum StartCommand {
    Node,
    BlockProducer,
    Rpc,
    Store,
    Faucet,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();

    match &cli.command {
        Command::Start { command, config } => match command {
            StartCommand::Node => start_node(config).await,
            StartCommand::BlockProducer => start_block_producer(config).await,
            StartCommand::Rpc => start_rpc(config).await,
            StartCommand::Store => start_store(config).await,
            StartCommand::Faucet => {
                start_faucet(config).await.map_err(|err| anyhow!("Faucet error: {err}"))
            },
        },
        Command::MakeGenesis { output_path, force, inputs_path } => {
            commands::make_genesis(inputs_path, output_path, force)
        },
    }
}
