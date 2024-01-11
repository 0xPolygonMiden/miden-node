use std::path::Path;

use anyhow::{anyhow, Result};
use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

/// The `(host, port)` pair for the server's listening socket.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Endpoint {
    /// Host used by the store.
    pub host: String,
    /// Port number used by the store.
    pub port: u16,
}

/// Loads the user configuration.
///
/// This function will look for the configuration file at the provided path. If the path is
/// relative, searches in parent directories all the way to the root as well.
///
/// The above configuration options are indented to support easy of packaging and deployment.
pub fn load_config(config_file: &Path) -> Figment {
    Figment::from(Toml::file(config_file))
}

/// Converts a hex string into a byte array.
///
/// This functionn will receive a hex_str as input, verify it's validity
/// and output a byte array containing it's raw data
///
/// Used in deserialisation of hex strings in configuration files
pub fn hex_string_to_byte_array<const N: usize>(hex_str: &str) -> Result<[u8; N]> {
    if !hex_str.starts_with("0x") {
        return Err(anyhow!("Seed should be formatted as hex strings starting with: '0x'"));
    }
    let raw_hex_data = &hex_str[2..];
    let mut bytes_array = [0u8; N];
    hex::decode_to_slice(raw_hex_data, &mut bytes_array)
        .map_err(|err| anyhow!("Error while processing hex string. {err}"))?;
    Ok(bytes_array)
}
