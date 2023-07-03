use std::fmt::Display;

use miden_node_store;
use serde::{Deserialize, Serialize};

pub const HOST: &str = "localhost";
// defined as: sum(ord(c)**p for (p, c) in enumerate('miden-rpc', 1)) % 2**16
pub const PORT: u16 = 57291;
pub const ENV_PREFIX: &str = "MIDEN_RPC";
pub const CONFIG_FILENAME: &str = "miden-rpc.toml";

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Endpoint {
    /// The host device the server will bind to.
    pub host: String,
    /// The port number to bind the server to.
    pub port: u16,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct RpcConfig {
    /// Main endpoint of the server.
    pub endpoint: Endpoint,
    /// Address of the store server in the format `http://<host>[:<port>]`.
    pub store: String,
}

impl Default for Endpoint {
    fn default() -> Self {
        Self {
            host: HOST.to_string(),
            port: PORT,
        }
    }
}

impl Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            endpoint: Endpoint::default(),
            store: format!("http://localhost:{}", miden_node_store::config::PORT),
        }
    }
}

impl miden_node_utils::Config for RpcConfig {
    const ENV_PREFIX: &'static str = ENV_PREFIX;
    const CONFIG_FILENAME: &'static str = CONFIG_FILENAME;
}
