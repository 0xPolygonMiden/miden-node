use std::{
    fmt::{Display, Formatter},
    path::PathBuf,
};

use miden_node_utils::config::Endpoint;
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
        self.endpoint.to_string()
    }
}

impl Display for StoreConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ endpoint: \"{}\",  database_filepath: {:?}, genesis_filepath: {:?} }}",
            self.endpoint, self.database_filepath, self.genesis_filepath
        ))
    }
}

// Top-level config
// ================================================================================================

/// Store top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct StoreTopLevelConfig {
    pub store: StoreConfig,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use figment::Jail;
    use miden_node_utils::config::load_config;

    use super::{Endpoint, StoreConfig, StoreTopLevelConfig};
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
