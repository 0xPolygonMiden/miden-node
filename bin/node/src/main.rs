// This is required due to a long chain of and_then in BlockBuilder::build_block causing rust error
// E0275.
#![recursion_limit = "256"]

use clap::{Parser, Subcommand};
use miden_node_utils::{logging::OpenTelemetry, version::LongVersion};

mod commands;

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
    /// Commands related to the node's store component.
    #[command(subcommand)]
    Store(commands::store::StoreCommand),

    /// Commands related to the node's RPC component.
    #[command(subcommand)]
    Rpc(commands::rpc::RpcCommand),

    /// Commands related to the node's block-producer component.
    #[command(subcommand)]
    BlockProducer(commands::block_producer::BlockProducerCommand),

    /// Commands relevant to running all components in the same process.
    ///
    /// This is the recommended way to run the node at the moment.
    #[command(subcommand)]
    Node(commands::node::NodeCommand),
}

impl Command {
    /// Whether OpenTelemetry tracing exporter should be enabled.
    ///
    /// This is enabled for some subcommands if the `--open-telemetry` flag is specified.
    fn open_telemetry(&self) -> OpenTelemetry {
        match self {
            Command::Store(subcommand) => subcommand.is_open_telemetry_enabled(),
            Command::Rpc(subcommand) => subcommand.is_open_telemetry_enabled(),
            Command::BlockProducer(subcommand) => subcommand.is_open_telemetry_enabled(),
            Command::Node(subcommand) => subcommand.is_open_telemetry_enabled(),
        }
        .then_some(OpenTelemetry::Enabled)
        .unwrap_or(OpenTelemetry::Disabled)
    }
}

// MAIN
// ================================================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Configure tracing with optional OpenTelemetry exporting support.
    miden_node_utils::logging::setup_tracing(cli.command.open_telemetry())?;

    match cli.command {
        Command::Rpc(rpc_command) => rpc_command.handle().await,
        Command::Store(store_command) => store_command.handle().await,
        Command::BlockProducer(block_producer_command) => block_producer_command.handle().await,
        Command::Node(node) => node.handle().await,
    }
}

// HELPERS & UTILITIES
// ================================================================================================

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
