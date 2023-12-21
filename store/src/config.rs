use std::path::PathBuf;

use miden_node_utils::config::{Config, Endpoint};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::genesis::DEFAULT_GENESIS_FILE_PATH;

/// The name of the organization - for config file path purposes
pub const ORG: &str = "Polygon";
/// The name of the app - for config file path purposes
pub const APP: &str = "Miden";

pub const HOST: &str = "localhost";
// defined as: sum(ord(c)**p for (p, c) in enumerate('miden-store', 1)) % 2**16
pub const PORT: u16 = 28943;
pub const CONFIG_FILENAME: &str = "miden-store.toml";
pub const STORE_FILENAME: &str = "miden-store.sqlite3";

pub static DEFAULT_STORE_PATH: Lazy<PathBuf> = Lazy::new(|| {
    directories::ProjectDirs::from("", ORG, APP)
        .map(|d| d.data_local_dir().join(STORE_FILENAME))
        // fallback to current dir
        .unwrap_or_default()
});

// Main config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Defines the lisening socket.
    pub endpoint: Endpoint,
    /// SQLite database file
    pub database_filepath: PathBuf,
    /// Genesis file
    pub genesis_filepath: PathBuf,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            endpoint: Endpoint {
                host: HOST.to_string(),
                port: PORT,
            },
            database_filepath: DEFAULT_STORE_PATH.clone(),
            genesis_filepath: DEFAULT_GENESIS_FILE_PATH.clone(),
        }
    }
}

impl StoreConfig {
    pub fn as_url(&self) -> String {
        format!("http://{}:{}", self.endpoint.host, self.endpoint.port)
    }
}

// Top-level config
// ================================================================================================

/// Store top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize, Default)]
pub struct StoreTopLevelConfig {
    pub store: StoreConfig,
}

impl Config for StoreTopLevelConfig {
    const CONFIG_FILENAME: &'static str = CONFIG_FILENAME;
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use figment::Jail;
    use miden_node_utils::Config;

    use super::{Endpoint, StoreConfig, StoreTopLevelConfig, CONFIG_FILENAME};

    #[test]
    fn test_store_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                CONFIG_FILENAME,
                r#"
                    [store]
                    database_filepath = "local.sqlite3"
                    genesis_filepath = "genesis.dat"

                    [store.endpoint]
                    host = "127.0.0.1"
                    port = 8080
                "#,
            )?;

            let config: StoreTopLevelConfig =
                StoreTopLevelConfig::load_config(Some(PathBuf::from(CONFIG_FILENAME).as_path()))
                    .extract()?;

            assert_eq!(
                config,
                StoreTopLevelConfig {
                    store: StoreConfig {
                        endpoint: Endpoint {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        database_filepath: "local.sqlite3".into(),
                        genesis_filepath: "genesis.dat".into()
                    }
                }
            );

            Ok(())
        });
    }
}
