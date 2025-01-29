use std::{
    fmt::{Display, Formatter},
    path::PathBuf,
};

use miden_node_utils::config::DEFAULT_STORE_PORT;
use serde::{Deserialize, Serialize};
use url::Url;

// Main config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreConfig {
    /// Defines the listening socket.
    pub endpoint: Url,
    /// `SQLite` database file
    pub database_filepath: PathBuf,
    /// Genesis file
    pub genesis_filepath: PathBuf,
    /// Block store directory
    pub blockstore_dir: PathBuf,
}

impl Display for StoreConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ endpoint: \"{}\",  database_filepath: {:?}, genesis_filepath: {:?}, blockstore_dir: {:?} }}",
            self.endpoint, self.database_filepath, self.genesis_filepath, self.blockstore_dir
        ))
    }
}

impl Default for StoreConfig {
    fn default() -> Self {
        const NODE_STORE_DIR: &str = "./";
        Self {
            endpoint: Url::parse(format!("127.0.0.1:{DEFAULT_STORE_PORT}").as_str()).unwrap(),
            database_filepath: PathBuf::from(NODE_STORE_DIR.to_string() + "miden-store.sqlite3"),
            genesis_filepath: PathBuf::from(NODE_STORE_DIR.to_string() + "genesis.dat"),
            blockstore_dir: PathBuf::from(NODE_STORE_DIR.to_string() + "blocks"),
        }
    }
}
