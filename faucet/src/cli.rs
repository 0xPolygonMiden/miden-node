use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(name = "Miden Faucet")]
#[clap(about = "A command line tool for the Miden faucet", long_about = None)]
pub struct Cli {
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

        #[clap(short, long)]
        asset_amount: u64,
    },

    /// Imports an existing Miden faucet from specified file
    Import {
        #[clap(short, long, required = true)]
        faucet_path: PathBuf,

        #[clap(short, long)]
        asset_amount: u64,
    },
}
