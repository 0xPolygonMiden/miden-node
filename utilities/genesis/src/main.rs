//! Generates a JSON file representing the chain state at genesis. This information will be used to
//! derive the genesis block.

mod state;

use anyhow::anyhow;
use clap::Parser;
use std::{fs::File, io::Write, path::Path};

use state::GenesisState;

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
        let genesis_state = GenesisState::default();

        serde_json::to_string_pretty(&genesis_state).expect("Failed to serialize genesis state")
    };

    let mut file = File::create(json_file_path)?;
    writeln!(file, "{}", genesis_state_json)?;

    Ok(())
}
