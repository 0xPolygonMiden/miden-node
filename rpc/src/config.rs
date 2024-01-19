use miden_node_utils::{config::Endpoint, control_plane::ControlPlaneConfig};
use serde::{Deserialize, Serialize};

pub const CONFIG_FILENAME: &str = "miden-rpc.toml";

// Main config
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct RpcConfig {
    pub endpoint: Endpoint,
    /// Store gRPC endpoint in the format `http://<host>[:<port>]`.
    pub store_url: String,
    /// Block producer gRPC endpoint in the format `http://<host>[:<port>]`.
    pub block_producer_url: String,
}

impl RpcConfig {
    pub fn as_url(&self) -> String {
        format!("http://{}:{}", self.endpoint.host, self.endpoint.port)
    }
}

// Top-level config
// ================================================================================================

/// Rpc top-level configuration.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct RpcTopLevelConfig {
    pub rpc: RpcConfig,
    pub control_plane: ControlPlaneConfig,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use figment::Jail;
    use miden_node_utils::{
        config::{load_config, Endpoint},
        control_plane::ControlPlaneConfig,
    };

    use super::{RpcConfig, RpcTopLevelConfig, CONFIG_FILENAME};

    #[test]
    fn test_rpc_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                CONFIG_FILENAME,
                r#"
                    [rpc]
                    store_url = "http://store:80"
                    block_producer_url = "http://block_producer:81"

                    [rpc.endpoint]
                    host = "0.0.0.0"
                    port = 82

                    [control_plane.endpoint]
                    host = "127.0.0.1"
                    port = 8080
                "#,
            )?;

            let config: RpcTopLevelConfig =
                load_config(PathBuf::from(CONFIG_FILENAME).as_path()).extract()?;

            assert_eq!(
                config,
                RpcTopLevelConfig {
                    rpc: RpcConfig {
                        endpoint: Endpoint {
                            host: "0.0.0.0".to_string(),
                            port: 82,
                        },
                        store_url: "http://store:80".to_string(),
                        block_producer_url: "http://block_producer:81".to_string(),
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
