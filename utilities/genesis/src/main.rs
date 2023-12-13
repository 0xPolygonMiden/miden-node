//! Generates a JSON file representing the chain state at genesis. This information will be used to
//! derive the genesis block.

mod state;

use anyhow::anyhow;
use std::{fs::write, path::Path};

use state::GenesisState;

const FILE_LOC: &'static str = "genesis.json";

fn main() -> anyhow::Result<()> {
    let json_file_path = Path::new(FILE_LOC);

    match json_file_path.try_exists() {
        Ok(existence_verified) => {
            if !existence_verified {
                return Err(anyhow!("Failed to generate new genesis file at {FILE_LOC}: there are broken symbolic links along the path"));
            }
        },
        Err(err) => {
            return Err(anyhow!("Failed to generate new genesis file at {FILE_LOC}. Error: {err}. Use the --force flag to overwrite."));
        },
    }

    let genesis_state_json = {
        let genesis_state = GenesisState::default();

        serde_json::to_string(&genesis_state).expect("Failed to serialize genesis state")
    };

    write(FILE_LOC, genesis_state_json)?;

    Ok(())
}
