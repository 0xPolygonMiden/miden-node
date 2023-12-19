use std::path::PathBuf;

use miden_node_utils::config::{Config, HostPort};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

pub const HOST: &str = "localhost";
// defined as: sum(ord(c)**p for (p, c) in enumerate('miden-store', 1)) % 2**16
pub const PORT: u16 = 28943;
pub const CONFIG_FILENAME: &str = "miden-store.toml";
pub const STORE_FILENAME: &str = "miden-store.sqlite3";

pub static DEFAULT_STORE_PATH: Lazy<PathBuf> = Lazy::new(|| {
    directories::ProjectDirs::from("", "Polygon", "Miden")
        .map(|d| d.data_local_dir().join(STORE_FILENAME))
        // fallback to current dir
        .unwrap_or_default()
});

// Main config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Defines the lisening socket.
    pub host_port: HostPort,
    /// SQLite database file
    pub sqlite: PathBuf,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            host_port: HostPort {
                host: HOST.to_string(),
                port: PORT,
            },
            sqlite: DEFAULT_STORE_PATH.clone(),
        }
    }
}

impl StoreConfig {
    pub fn as_endpoint(&self) -> String {
        format!("http://{}:{}", self.host_port.host, self.host_port.port)
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
    use figment::Jail;
    use miden_node_utils::Config;

    use super::{HostPort, StoreConfig, StoreTopLevelConfig, CONFIG_FILENAME};

    #[test]
    fn test_store_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                CONFIG_FILENAME,
                r#"
                    [store]
                    sqlite = "local.sqlite3"

                    [store.host_port]
                    host = "127.0.0.1"
                    port = 8080
                "#,
            )?;

            let config: StoreTopLevelConfig = StoreTopLevelConfig::load_config(None).extract()?;

            assert_eq!(
                config,
                StoreTopLevelConfig {
                    store: StoreConfig {
                        host_port: HostPort {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                        sqlite: "local.sqlite3".into(),
                    }
                }
            );

            Ok(())
        });
    }
}
