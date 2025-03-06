// This is required due to a long chain of and_then in BlockBuilder::build_block causing rust error
// E0275.
#![recursion_limit = "256"]

use std::path::PathBuf;

use anyhow::{Context, anyhow};
use clap::{Parser, Subcommand};
use commands::{
    block_producer::BlockProducerCommand, init::init_config_files, node::NodeCommand,
    rpc::RpcCommand, start::start_node, store::StoreCommand,
};
use miden_node_block_producer::server::BlockProducer;
use miden_node_rpc::server::Rpc;
use miden_node_store::server::Store;
use miden_node_utils::{config::load_config, logging::OpenTelemetry, version::LongVersion};

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
#[command(version, about, long_about = None, long_version = long_version().to_string())]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(subcommand)]
    Store(commands::store::StoreCommand),
    #[command(subcommand)]
    Rpc(commands::rpc::RpcCommand),
    #[command(subcommand)]
    BlockProducer(commands::block_producer::BlockProducerCommand),
    #[command(subcommand)]
    Node(commands::node::NodeCommand),

    /// Start the node
    Start {
        #[command(subcommand)]
        command: StartCommand,

        #[arg(short, long, value_name = "FILE", default_value = NODE_CONFIG_FILE_PATH)]
        config: PathBuf,

        #[arg(long = "open-telemetry", default_value_t = false)]
        open_telemetry: bool,
    },

    /// Generates a genesis file and associated account files based on a specified genesis input
    ///
    /// This command creates a new genesis file and associated account files at the specified
    /// output paths. It checks for the existence of the output file, and if it already exists,
    /// an error is thrown unless the `force` flag is set to overwrite it.
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
    /// This command creates two files (miden-node.toml and genesis.toml) that provide
    /// configuration details to the node. These files may be modified to change the node
    /// behavior.
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Open telemetry exporting is only valid for running the node.
    let open_telemetry = if let Command::Start { open_telemetry: true, .. } = &cli.command {
        OpenTelemetry::Enabled
    } else {
        OpenTelemetry::Disabled
    };
    miden_node_utils::logging::setup_tracing(open_telemetry)?;

    match cli.command {
        Command::Start { command, config, .. } => match command {
            StartCommand::Node => {
                let config = load_config(config).context("Loading configuration file")?;
                start_node(config).await
            },
        },
        Command::MakeGenesis { output_path, force, inputs_path } => {
            commands::make_genesis(&inputs_path, &output_path, force)
        },
        Command::Init { config_path, genesis_path } => {
            let current_dir = std::env::current_dir()
                .map_err(|err| anyhow!("failed to open current directory: {err}"))?;

            let config = current_dir.join(config_path);
            let genesis = current_dir.join(genesis_path);

            init_config_files(&config, &genesis)
        },
        Command::Rpc(RpcCommand::Start { url, store_url, block_producer_url }) => {
            Rpc::init(url, store_url, block_producer_url)
                .await
                .context("Loading RPC")?
                .serve()
                .await
                .context("Serving RPC")
        },
        Command::Store(StoreCommand::Init) => todo!(),
        Command::Store(StoreCommand::Start { url, data_directory, genesis_filepath }) => {
            Store::init(url, data_directory, genesis_filepath)
                .await
                .context("Loading store")?
                .serve()
                .await
                .context("Serving store")
        },
        Command::BlockProducer(BlockProducerCommand::Start { url, store_url }) => {
            BlockProducer::init(url, store_url)
                .await
                .context("Loading block-producer")?
                .serve()
                .await
                .context("Serving block-producer")
        },
        Command::Node(NodeCommand::Start { rpc_url, store_data_directory }) => todo!(),
    }
}

/// Generates [`LongVersion`] using the metadata generated by build.rs.
fn long_version() -> LongVersion {
    LongVersion {
        version: env!("CARGO_PKG_VERSION"),
        sha: option_env!("VERGEN_GIT_SHA").unwrap_or_default(),
        branch: option_env!("VERGEN_GIT_BRANCH").unwrap_or_default(),
        dirty: option_env!("VERGEN_GIT_DIRTY").unwrap_or_default(),
        features: option_env!("VERGEN_CARGO_FEATURES").unwrap_or_default(),
        rust_version: option_env!("VERGEN_RUSTC_SEMVER").unwrap_or_default(),
        host: option_env!("VERGEN_RUSTC_HOST_TRIPLE").unwrap_or_default(),
        target: option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or_default(),
        opt_level: option_env!("VERGEN_CARGO_OPT_LEVEL").unwrap_or_default(),
        debug: option_env!("VERGEN_CARGO_DEBUG").unwrap_or_default(),
    }
}
