use std::{fs::File, io::Write, path::Path};

use anyhow::{anyhow, Result};

use crate::{commands::genesis::GenesisInput, config::NodeConfig};

// INIT
// ===================================================================================================

pub fn init_config_files(config_file_path: &Path, genesis_file_path: &Path) -> Result<()> {
    let config = NodeConfig::default();
    let config_as_toml_string = toml::to_string(&config)
        .map_err(|err| anyhow!("Failed to serialize default config: {}", err))?;

    write_string_in_file(&config_as_toml_string, config_file_path)?;

    println!("Config file successfully created at: {config_file_path:?}");

    let genesis = GenesisInput::default();
    let genesis_as_toml_string = toml::to_string(&genesis)
        .map_err(|err| anyhow!("Failed to serialize default config: {}", err))?;

    write_string_in_file(&genesis_as_toml_string, genesis_file_path)?;

    println!("Genesis config file successfully created at: {genesis_file_path:?}");

    Ok(())
}

fn write_string_in_file(content: &str, path: &Path) -> Result<()> {
    let mut file_handle = File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| anyhow!("Error opening the file: {err}"))?;

    file_handle
        .write(content.as_bytes())
        .map_err(|err| anyhow!("Error writing to file: {err}"))?;

    Ok(())
}
