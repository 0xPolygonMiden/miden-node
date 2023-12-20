use std::path::Path;

use directories;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

/// Environment variable default prefix.
pub const ENV_PREFIX: &str = "MIDEN__";

/// The `(host, port)` pair for the server's listening socket.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Endpoint {
    /// Host used by the store.
    pub host: String,
    /// Port number used by the store.
    pub port: u16,
}

/// Trait with the basic logic to load configurations for different services.
///
/// This trait makes sure the priority and features are consistent across different services.
pub trait Config: Default + Serialize {
    const CONFIG_FILENAME: &'static str;
    const ENV_PREFIX: &'static str = ENV_PREFIX;

    fn load_user_config() -> Option<Figment> {
        let dirs = directories::ProjectDirs::from("", "Polygon", "Miden")?;
        let file = dirs.config_local_dir().join(Self::CONFIG_FILENAME);

        match file.exists() {
            true => Some(Figment::from(Toml::file(file))),
            false => None,
        }
    }

    fn load_local_config(config: Option<&Path>) -> Figment {
        Figment::from(Toml::file(config.unwrap_or(Path::new(Self::CONFIG_FILENAME))))
    }

    fn load_env_config() -> Figment {
        Figment::from(Env::prefixed(Self::ENV_PREFIX).split("__"))
    }

    /// Loads the user configuration.
    ///
    /// This function will look on the following places, from lowest to higest priority:
    ///
    /// - Code defaults
    /// - User's configuration file on the system's default locations.
    /// - Configuration file on the current directory or the config file provided via CLI arg.
    /// - Environment variables.
    ///
    /// The above configuration options are indented to support easy of packaging and deployment.
    fn load_config(config_file: Option<&Path>) -> Figment {
        Figment::from(Serialized::defaults(Self::default()))
            .merge(Self::load_user_config().unwrap_or_default())
            .merge(Self::load_local_config(config_file))
            .merge(Self::load_env_config())
    }
}
