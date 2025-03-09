use miden_node_block_producer::config::BlockProducerConfig;
use miden_node_rpc::config::RpcConfig;
use miden_node_store::config::StoreConfig;
use serde::{Deserialize, Serialize};
use url::Url;

/// Node top-level configuration.
#[derive(Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeConfig {
    block_producer: NormalizedBlockProducerConfig,
    rpc: NormalizedRpcConfig,
    store: StoreConfig,
}

/// A specialized variant of [`RpcConfig`] with redundant fields within [`NodeConfig`] removed.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NormalizedRpcConfig {
    endpoint: Url,
}

/// A specialized variant of [`BlockProducerConfig`] with redundant fields within [`NodeConfig`]
/// removed.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NormalizedBlockProducerConfig {
    endpoint: Url,
    verify_tx_proofs: bool,
    batch_prover_url: Option<Url>,
    block_prover_url: Option<Url>,
}

impl Default for NormalizedRpcConfig {
    fn default() -> Self {
        // Ensure we stay in sync with the original defaults.
        let RpcConfig {
            endpoint,
            store_url: _,
            block_producer_url: _,
        } = RpcConfig::default();
        Self { endpoint }
    }
}

impl Default for NormalizedBlockProducerConfig {
    fn default() -> Self {
        // Ensure we stay in sync with the original defaults.
        let BlockProducerConfig {
            endpoint,
            store_url: _,
            verify_tx_proofs,
            batch_prover_url,
            block_prover_url,
        } = BlockProducerConfig::default();
        Self {
            endpoint,
            verify_tx_proofs,
            batch_prover_url,
            block_prover_url,
        }
    }
}

impl NodeConfig {
    pub fn into_parts(self) -> (BlockProducerConfig, RpcConfig, StoreConfig) {
        let Self { block_producer, rpc, store } = self;

        let block_producer = BlockProducerConfig {
            endpoint: block_producer.endpoint,
            store_url: store.endpoint.clone(),
            verify_tx_proofs: block_producer.verify_tx_proofs,
            batch_prover_url: block_producer.batch_prover_url,
            block_prover_url: block_producer.block_prover_url,
        };

        let rpc = RpcConfig {
            endpoint: rpc.endpoint,
            store_url: store.endpoint.clone(),
            block_producer_url: block_producer.endpoint.clone(),
        };

        (block_producer, rpc, store)
    }
}

#[cfg(test)]
mod tests {
    use figment::Jail;
    use miden_node_store::config::StoreConfig;
    use miden_node_utils::config::load_config;
    use url::Url;

    use super::NodeConfig;
    use crate::{
        NODE_CONFIG_FILE_PATH,
        config::{NormalizedBlockProducerConfig, NormalizedRpcConfig},
    };

    #[test]
    fn node_config() {
        Jail::expect_with(|jail| {
            jail.create_file(
                NODE_CONFIG_FILE_PATH,
                r#"
                    [block_producer]
                    endpoint = "http://127.0.0.1:8080"
                    verify_tx_proofs = true
                    batch_prover_url = "http://127.0.0.1:8081"
                    block_prover_url = "http://127.0.0.1:8082"

                    [rpc]
                    endpoint = "http://127.0.0.1:8080"

                    [store]
                    endpoint = "https://127.0.0.1:8080"
                    database_filepath = "local.sqlite3"
                    genesis_filepath = "genesis.dat"
                    blockstore_dir = "blocks"
                "#,
            )?;

            let config: NodeConfig = load_config(NODE_CONFIG_FILE_PATH)?;

            assert_eq!(
                config,
                NodeConfig {
                    block_producer: NormalizedBlockProducerConfig {
                        endpoint: Url::parse("http://127.0.0.1:8080").unwrap(),
                        verify_tx_proofs: true,
                        batch_prover_url: Some(Url::parse("http://127.0.0.1:8081").unwrap()),
                        block_prover_url: Some(Url::parse("http://127.0.0.1:8082").unwrap()),
                    },
                    rpc: NormalizedRpcConfig {
                        endpoint: Url::parse("http://127.0.0.1:8080").unwrap(),
                    },
                    store: StoreConfig {
                        endpoint: Url::parse("https://127.0.0.1:8080").unwrap(),
                        database_filepath: "local.sqlite3".into(),
                        genesis_filepath: "genesis.dat".into(),
                        blockstore_dir: "blocks".into()
                    },
                }
            );

            Ok(())
        });
    }
}
