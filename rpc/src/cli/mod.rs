use std::path::PathBuf;

use clap::{Parser, Subcommand};
use hex::FromHex;
use miden_node_proto::digest::Digest;
use miden_node_rpc::config;

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
    Serve,

    #[command(subcommand)]
    Request(Request),
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Subcommand)]
pub enum Request {
    CheckNullifiers {
        #[arg(value_parser=parse_nullifier)]
        /// List of nullifiers to check
        nullifiers: Vec<Digest>,
    },
}

fn parse_nullifier(value: &str) -> Result<Digest, String> {
    Digest::from_hex(value.as_bytes()).map_err(|e| e.to_string())
}
