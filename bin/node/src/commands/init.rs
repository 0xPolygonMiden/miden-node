use std::{fs::File, io::Write, path::PathBuf};

use anyhow::{anyhow, Result};

use crate::config::NodeConfig;

// INIT
// ===================================================================================================

pub fn init_config_files(config_file_path: PathBuf, _genesis_file_path: PathBuf) -> Result<()> {
    let config = NodeConfig::default();
    let config_as_toml_string = toml::to_string(&config)
        .map_err(|err| anyhow!("Failed to serialize default config: {}", err))?;

    let mut file_handle = File::options()
        .write(true)
        .create_new(true)
        .open(&config_file_path)
        .map_err(|err| anyhow!("Error opening the file: {err}"))?;

    file_handle
        .write(config_as_toml_string.as_bytes())
        .map_err(|err| anyhow!("Error writing to file: {err}"))?;

    println!("Config file successfully created at: {:?}", config_file_path);

    Ok(())
}
