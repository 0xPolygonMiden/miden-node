use std::path::Path;

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
/// This function will look on the following places, from lowest to higest priority:
///
/// - Configuration file at the provided path. If the path is relative, searches in parent
///   directories all the way to the root as well.
/// - Environment variables.
///
/// The above configuration options are indented to support easy of packaging and deployment.
pub fn load_config(config_file: &Path) -> Figment {
    Figment::from(Toml::file(config_file))
}
