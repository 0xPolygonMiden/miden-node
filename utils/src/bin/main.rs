//! Generates a JSON file representing the chain state at genesis. This information will be used to
//! derive the genesis block.

use std::{
    fs::File,
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use clap::Parser;
use miden_crypto::{dsa::rpo_falcon512::PublicKey, Felt};
use miden_lib::{faucets::create_basic_fungible_faucet, wallets::create_basic_wallet, AuthScheme};
use miden_node_utils::genesis::GenesisState;
use miden_objects::assets::TokenSymbol;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to output json file
    #[arg(short, long, default_value = "genesis.json")]
    output_path: String,

    /// Generate the output file even if a file already exists
    #[arg(short, long)]
    force: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let json_file_path = Path::new(&args.output_path);

    if !args.force {
        match json_file_path.try_exists() {
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

    let genesis_state = {
        let pub_key = PublicKey::new([0; 897]).unwrap();
        let accounts = {
            let mut accounts = Vec::new();

            // fungible asset faucet
            {
                let (account, _) = create_basic_fungible_faucet(
                    [0; 32],
                    TokenSymbol::new("TODO").unwrap(),
                    9,
                    Felt::from(1_000_000_000_u64),
                    AuthScheme::RpoFalcon512 { pub_key },
                )
                .unwrap();

                accounts.push(account);
            }

            // basic wallet account
            {
                let (account, _) = create_basic_wallet(
                    [0; 32],
                    AuthScheme::RpoFalcon512 { pub_key },
                    miden_objects::accounts::AccountType::RegularAccountUpdatableCode,
                )
                .unwrap();

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

    let genesis_state_json =
        serde_json::to_string_pretty(&genesis_state).expect("Failed to serialize genesis state");

    let mut file = File::create(json_file_path)?;
    writeln!(file, "{}", genesis_state_json)?;

    Ok(())
}
