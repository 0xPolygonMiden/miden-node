use std::{
    fmt::{Display, Formatter},
    io,
    net::{SocketAddr, ToSocketAddrs},
    path::Path,
    vec,
};

use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_NODE_RPC_PORT: u16 = 57291;
pub const DEFAULT_BLOCK_PRODUCER_PORT: u16 = 48046;
pub const DEFAULT_STORE_PORT: u16 = 28943;
pub const DEFAULT_FAUCET_SERVER_PORT: u16 = 8080;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum Protocol {
    Http,
    Https,
}
/// The `(host, port)` pair for the server's listening socket.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Endpoint {
    /// Host used by the store.
    pub host: String,
    /// Port number used by the store.
    pub port: u16,
    /// Protocol type: http or https.
    pub protocol: Protocol,
}

impl Endpoint {
    pub fn localhost(port: u16) -> Self {
        Endpoint {
            host: "localhost".to_string(),
            port,
            protocol: Protocol::Http,
        }
    }
}

impl ToSocketAddrs for Endpoint {
    type Iter = vec::IntoIter<SocketAddr>;
    fn to_socket_addrs(&self) -> io::Result<Self::Iter> {
        (self.host.as_ref(), self.port).to_socket_addrs()
    }
}

impl Display for Endpoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.protocol {
            Protocol::Http => f.write_fmt(format_args!("http://{}:{}", self.host, self.port)),
            Protocol::Https => f.write_fmt(format_args!("https://{}:{}", self.host, self.port)),
        }
    }
}

/// Loads the user configuration.
///
/// This function will look for the configuration file at the provided path. If the path is
/// relative, searches in parent directories all the way to the root as well.
///
/// The above configuration options are indented to support easy of packaging and deployment.
pub fn load_config<T: for<'a> Deserialize<'a>>(
    config_file: impl AsRef<Path>,
) -> figment::Result<T> {
    Figment::from(Toml::file(config_file.as_ref())).extract()
}
