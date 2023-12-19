use std::{
    fmt::{Display, Formatter},
    path::PathBuf,
    str::FromStr,
};

use clap::{Parser, Subcommand};
use miden_node_store::genesis::DEFAULT_GENESIS_FILE_PATH;

mod commands;
mod config;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE", default_value = config::CONFIG_FILENAME)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the node
    Start,

    /// Generate genesis file
    MakeGenesis {
        #[arg(short, long, default_value_t = DEFAULT_GENESIS_FILE_PATH.clone().into())]
        output_path: DisplayPathBuf,

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
        Command::Start => commands::start().await,
        Command::MakeGenesis { output_path, force } => {
            commands::make_genesis(output_path, force).await
        },
    }
}

// HELPERS
// =================================================================================================

/// This type is needed for use as a `clap::Arg`. The problem with `PathBuf` is that it doesn't
/// implement `Display`; this is a thin wrapper around `PathBuf` which does implement `Display`
#[derive(Debug, Clone)]
pub struct DisplayPathBuf(PathBuf);

impl Display for DisplayPathBuf {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl From<PathBuf> for DisplayPathBuf {
    fn from(value: PathBuf) -> Self {
        Self(value)
    }
}

impl FromStr for DisplayPathBuf {
    type Err = <PathBuf as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(PathBuf::from_str(s)?))
    }
}
