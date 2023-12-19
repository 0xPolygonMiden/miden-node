//! Generates a JSON file representing the chain state at genesis. This information will be used to
//! derive the genesis block.

use std::{
    fmt::{Display, Formatter},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use clap::Parser;
use miden_crypto::{dsa::rpo_falcon512::KeyPair, utils::Serializable, Felt};
use miden_lib::{faucets::create_basic_fungible_faucet, wallets::create_basic_wallet, AuthScheme};
use miden_node_utils::genesis::{GenesisState, DEFAULT_GENESIS_FILE_PATH};
use miden_objects::assets::TokenSymbol;

// CONSTANTS
// =================================================================================================

/// Token symbol of the faucet present at genesis
const FUNGIBLE_FAUCET_TOKEN_SYMBOL: &str = "POL";

/// Decimals for the token of the faucet present at genesis
const FUNGIBLE_FAUCET_TOKEN_DECIMALS: u8 = 9;

/// Max supply for the token of the faucet present at genesis
const FUNGIBLE_FAUCET_TOKEN_MAX_SUPPLY: u64 = 1_000_000_000;

/// Seed for the Falcon512 keypair (faucet account)
const SEED_FAUCET_KEYPAIR: [u8; 40] = [2_u8; 40];

/// Seed for the Falcon512 keypair (wallet account)
const SEED_WALLET_KEYPAIR: [u8; 40] = [3_u8; 40];

/// Seed for the fungible faucet account
const SEED_FAUCET: [u8; 32] = [0_u8; 32];

/// Seed for the basic wallet account
const SEED_WALLET: [u8; 32] = [1_u8; 32];

/// Faucet account keys (public/private) file path
const FAUCET_KEYPAIR_FILE_PATH: &str = "faucet.fsk";

/// Wallet account keys (public/private) file path
const WALLET_KEYPAIR_FILE_PATH: &str = "wallet.fsk";

// MAIN
// =================================================================================================

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to output json file
    #[arg(short, long, default_value_t = DEFAULT_GENESIS_FILE_PATH.clone().into())]
    output_path: DisplayPathBuf,

    /// Generate the output file even if a file already exists
    #[arg(short, long)]
    force: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let output_file_path = Path::new(&args.output_path.0);

    if !args.force {
        match output_file_path.try_exists() {
            Ok(file_exists) => {
                if file_exists {
                    return Err(anyhow!("Failed to generate new genesis file \"{}\" because it already exists. Use the --force flag to overwrite.", args.output_path));
                }
            },
            Err(err) => {
                return Err(anyhow!(
                    "Failed to generate new genesis file \"{}\". Error: {err}",
                    args.output_path
                ));
            },
        }
    }

    let faucet_key_pair = KeyPair::from_seed(&SEED_FAUCET_KEYPAIR).unwrap();
    let wallet_key_pair = KeyPair::from_seed(&SEED_WALLET_KEYPAIR).unwrap();

    let genesis_state = {
        let accounts = {
            let mut accounts = Vec::new();

            // fungible asset faucet
            {
                println!("Generating faucet account... ");
                let (account, _) = create_basic_fungible_faucet(
                    SEED_FAUCET,
                    TokenSymbol::new(FUNGIBLE_FAUCET_TOKEN_SYMBOL).unwrap(),
                    FUNGIBLE_FAUCET_TOKEN_DECIMALS,
                    Felt::from(FUNGIBLE_FAUCET_TOKEN_MAX_SUPPLY),
                    AuthScheme::RpoFalcon512 {
                        pub_key: faucet_key_pair.public_key(),
                    },
                )
                .unwrap();

                println!("Done");

                accounts.push(account);
            }

            // basic wallet account
            {
                println!("Generating basic wallet account... ");
                let (account, _) = create_basic_wallet(
                    SEED_WALLET,
                    AuthScheme::RpoFalcon512 {
                        pub_key: wallet_key_pair.public_key(),
                    },
                    miden_objects::accounts::AccountType::RegularAccountUpdatableCode,
                )
                .unwrap();

                println!("Done");

                accounts.push(account);
            }

            accounts
        };

        let version = 0;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("we are after 1970")
            .as_millis() as u64;

        GenesisState::new(accounts, version, timestamp)
    };

    // Write genesis state as binary format
    {
        let genesis_state_bin = genesis_state.to_bytes();

        fs::write(output_file_path, genesis_state_bin).unwrap_or_else(|_| {
            panic!("Failed to write genesis state to output file {}", output_file_path.display())
        });
    }

    // Write keypairs to disk
    fs::write(FAUCET_KEYPAIR_FILE_PATH, faucet_key_pair.to_bytes()).unwrap();
    fs::write(WALLET_KEYPAIR_FILE_PATH, wallet_key_pair.to_bytes()).unwrap();

    Ok(())
}

// HELPERS
// =================================================================================================

/// This type is needed for use as a `clap::Arg`. The problem with `PathBuf` is that it doesn't
/// implement `Display`; this is a thin wrapper around `PathBuf` which does implement `Display`
#[derive(Debug, Clone)]
struct DisplayPathBuf(PathBuf);

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
