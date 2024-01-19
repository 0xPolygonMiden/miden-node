use std::path::PathBuf;

use miden_node_utils::{config::Endpoint, control_plane::ControlPlaneConfig};
use serde::{Deserialize, Serialize};

pub const CONFIG_FILENAME: &str = "miden-store.toml";

// Main config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Defines the listening socket.
    pub endpoint: Endpoint,
    /// SQLite database file
    pub database_filepath: PathBuf,
    /// Genesis file
    pub genesis_filepath: PathBuf,
}

impl StoreConfig {
    pub fn as_url(&self) -> String {
        format!("http://{}:{}", self.endpoint.host, self.endpoint.port)
    }
}

// Top-level config
// ================================================================================================

/// Store top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct StoreTopLevelConfig {
    pub store: StoreConfig,
    pub control_plane: ControlPlaneConfig,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use figment::Jail;
    use miden_node_utils::config::load_config;

    use super::{ControlPlaneConfig, Endpoint, StoreConfig, StoreTopLevelConfig};
    use crate::config::CONFIG_FILENAME;

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
                    host = "0.0.0.0"
                    port = 80

                    [control_plane.endpoint]
                    host = "127.0.0.1"
                    port = 8080
                "#,
            )?;

            let config: StoreTopLevelConfig =
                load_config(PathBuf::from(CONFIG_FILENAME).as_path()).extract()?;

            assert_eq!(
                config,
                StoreTopLevelConfig {
                    store: StoreConfig {
                        endpoint: Endpoint {
                            host: "0.0.0.0".to_string(),
                            port: 80,
                        },
                        database_filepath: "local.sqlite3".into(),
                        genesis_filepath: "genesis.dat".into()
                    },
                    control_plane: ControlPlaneConfig {
                        endpoint: Endpoint {
                            host: "127.0.0.1".to_string(),
                            port: 8080,
                        },
                    }
                }
            );

            Ok(())
        });
    }
}
