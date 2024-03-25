use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::config;

#[derive(Parser)]
#[clap(name = "Miden Faucet")]
#[clap(about = "A command line tool for the Miden faucet", long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE", default_value = config::CONFIG_FILENAME)]
    pub config: PathBuf,

    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialise a new Miden faucet from arguments
    Init {
        #[clap(short, long, required = true)]
        token_symbol: String,

        #[clap(short, long, required = true)]
        decimals: u8,

        #[clap(short, long, required = true)]
        max_supply: u64,

        /// Amount of assets to be dispersed by the faucet on each request
        #[clap(short, long)]
        asset_amount: u64,
    },

    /// Imports an existing Miden faucet from specified file
    Import {
        #[clap(short, long, required = true)]
        faucet_path: PathBuf,

        /// Amount of assets to be dispersed by the faucet on each request
        #[clap(short, long)]
        asset_amount: u64,
    },
}
