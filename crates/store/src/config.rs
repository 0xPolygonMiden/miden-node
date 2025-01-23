use std::{
    fmt::{Display, Formatter},
    path::PathBuf,
};

use miden_node_utils::config::{Endpoint, DEFAULT_STORE_PORT};
use serde::{Deserialize, Serialize};

// Main config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreConfig {
    /// Defines the listening socket.
    pub endpoint: Endpoint,
    /// SQLite database file
    pub database_filepath: PathBuf,
    /// Genesis file
    pub genesis_filepath: PathBuf,
    /// Block store directory
    pub blockstore_dir: PathBuf,
    /// Account SMT tree updates store directory
    pub account_smt_updates_dir: PathBuf,
}

impl StoreConfig {
    pub fn endpoint_url(&self) -> String {
        self.endpoint.to_string()
    }
}

impl Display for StoreConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ endpoint: \"{}\",  database_filepath: {:?}, genesis_filepath: {:?}, blockstore_dir: {:?}, account_smt_updates_dir: {:?} }}",
            self.endpoint, self.database_filepath, self.genesis_filepath, self.blockstore_dir, self.account_smt_updates_dir
        ))
    }
}

impl Default for StoreConfig {
    fn default() -> Self {
        const STORAGE_DIR: &str = "./storage";
        Self {
            endpoint: Endpoint::localhost(DEFAULT_STORE_PORT),
            database_filepath: PathBuf::from(format!("{STORAGE_DIR}/miden-store.sqlite3")),
            genesis_filepath: PathBuf::from(format!("{STORAGE_DIR}/genesis.dat")),
            blockstore_dir: PathBuf::from(format!("{STORAGE_DIR}/blocks")),
            account_smt_updates_dir: PathBuf::from(format!("{STORAGE_DIR}/account-smt-updates")),
        }
    }
}
