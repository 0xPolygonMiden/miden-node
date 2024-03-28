use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::config;

#[derive(Parser, Debug)]
#[clap(name = "Miden Faucet")]
#[clap(about = "A command line tool for the Miden faucet", long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialise a new Miden faucet from arguments
    Init(InitArgs),

    /// Imports an existing Miden faucet from specified file
    Import(ImportArgs),
}

#[derive(Parser, Debug)]
pub struct InitArgs {
    #[clap(short, long)]
    pub token_symbol: String,

    #[clap(short, long)]
    pub decimals: u8,

    #[clap(short, long)]
    pub max_supply: u64,

    /// Amount of assets to be dispersed by the faucet on each request
    #[clap(short, long)]
    pub asset_amount: u64,

    #[clap(short, long, value_name = "FILE", default_value = config::CONFIG_FILENAME)]
    pub config: PathBuf,
}

#[derive(Parser, Debug)]
pub struct ImportArgs {
    #[clap(short, long)]
    pub faucet_path: PathBuf,

    /// Amount of assets to be dispersed by the faucet on each request
    #[clap(short, long)]
    pub asset_amount: u64,

    #[clap(short, long, value_name = "FILE", default_value = config::CONFIG_FILENAME)]
    pub config: PathBuf,
}
