//! Generates a JSON file representing the chain state at genesis. This information will be used to
//! derive the genesis block.

mod state;

use anyhow::anyhow;
use std::{fs::File, io::Write, path::Path};

use state::GenesisState;

const FILE_LOC: &'static str = "genesis.json";

fn main() -> anyhow::Result<()> {
    let json_file_path = Path::new(FILE_LOC);

    match json_file_path.try_exists() {
        Ok(file_exists) => {
            if file_exists {
                return Err(anyhow!("Failed to generate new genesis file \"{FILE_LOC}\" because it already exists. Use the --force flag to overwrite."));
            }
        },
        Err(err) => {
            return Err(anyhow!("Failed to generate new genesis file \"{FILE_LOC}\". Error: {err}"));
        },
    }

    let genesis_state_json = {
        let genesis_state = GenesisState::default();

        serde_json::to_string_pretty(&genesis_state).expect("Failed to serialize genesis state")
    };

    let mut file = File::create(FILE_LOC)?;
    writeln!(file, "{}", genesis_state_json)?;

    Ok(())
}
