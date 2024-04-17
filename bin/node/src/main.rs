use std::path::PathBuf;

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use commands::{start_block_producer, start_node, start_rpc, start_store};
use miden_node_block_producer::config::BlockProducerConfig;
// use miden_node_faucet::{config::FaucetConfig, start_faucet};
use miden_node_rpc::config::RpcConfig;
use miden_node_store::config::StoreConfig;
use miden_node_utils::config::load_config;
use serde::{Deserialize, Serialize};

mod commands;

// Top-level config
// ================================================================================================

/// Node top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct StartCommandConfig {
    pub block_producer: BlockProducerConfig,
    pub rpc: RpcConfig,
    pub store: StoreConfig,
    // pub fauecet: FaucetConfig
}

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
        Command::Start { command, config } => {
            let config: StartCommandConfig = load_config(config).extract().map_err(|err| {
                anyhow!("failed to load config file `{}`: {err}", config.display())
            })?;
            match command {
                StartCommand::Node => start_node(config).await,
                StartCommand::BlockProducer => start_block_producer(config.block_producer).await,
                StartCommand::Rpc => start_rpc(config.rpc).await,
                StartCommand::Store => start_store(config.store).await,
                StartCommand::Faucet => {
                    // start_faucet(config).await.map_err(|err| anyhow!("Faucet error: {err}"))
                    Ok(())
                },
            }
        },
        Command::MakeGenesis { output_path, force, inputs_path } => {
            commands::make_genesis(inputs_path, output_path, force)
        },
    }
}
