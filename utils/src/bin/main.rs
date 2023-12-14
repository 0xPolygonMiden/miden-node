//! Generates a JSON file representing the chain state at genesis. This information will be used to
//! derive the genesis block.

use std::{fs::File, io::Write, path::Path};

use anyhow::anyhow;
use clap::Parser;
use miden_crypto::dsa::rpo_falcon512::PublicKey;
use miden_node_utils::genesis::GenesisState;

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

    let genesis_state_json = {
        // FIXME: Which pubkey to use?
        let pub_key = PublicKey::new([0; 897]).unwrap();
        let genesis_state = GenesisState::new(pub_key);

        serde_json::to_string_pretty(&genesis_state).expect("Failed to serialize genesis state")
    };

    let mut file = File::create(json_file_path)?;
    writeln!(file, "{}", genesis_state_json)?;

    Ok(())
}
