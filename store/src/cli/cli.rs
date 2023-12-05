use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::config;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE", default_value = config::CONFIG_FILENAME)]
    pub config: Option<PathBuf>,

    #[arg(short, long, value_name = "SQLITE_FILE")]
    pub sqlite: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Subcommand)]
pub enum Command {
    Serve {
        #[arg(short, long)]
        /// Binding port number
        port: Option<u16>,

        // short option `-h` conflicts with `--help`, so it is not enabled.
        #[arg(long)]
        /// Binding host
        host: Option<String>,
    },
}
