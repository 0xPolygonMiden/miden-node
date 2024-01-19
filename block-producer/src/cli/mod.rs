use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::config;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE", default_value = config::CONFIG_FILENAME)]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Subcommand)]
pub enum Command {
    /// Starts the block producer gRPC service.
    Serve,
}
