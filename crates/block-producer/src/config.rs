use std::fmt::{Display, Formatter};

use miden_node_utils::config::{DEFAULT_BLOCK_PRODUCER_PORT, DEFAULT_STORE_PORT};
use serde::{Deserialize, Serialize};
use url::Url;

// Main config
// ================================================================================================

/// Block producer specific configuration
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockProducerConfig {
    pub endpoint: Url,

    /// Store gRPC endpoint in the format `http://<host>[:<port>]`.
    pub store_url: Url,

    /// Enable or disable the verification of transaction proofs before they are accepted into the
    /// transaction queue.
    ///
    /// Disabling transaction proof verification will speed up transaction processing as proof
    /// verification may take ~15ms/proof. This is OK when all transactions are forwarded to the
    /// block producer from the RPC component as transaction proofs are also verified there.
    pub verify_tx_proofs: bool,

    /// URL of the remote batch prover.
    ///
    /// If not set, the block producer will use the local batch prover.
    pub batch_prover_url: Option<Url>,

    /// URL of the remote block prover.
    ///
    /// If not set, the block producer will use the local block prover.
    pub block_prover_url: Option<Url>,
}

impl Display for BlockProducerConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ endpoint: \"{}\"", self.endpoint)?;
        write!(f, ", store_url: \"{}\"", self.store_url)?;

        let batch_prover_url = self
            .batch_prover_url
            .as_ref()
            .map_or_else(|| "None".to_string(), ToString::to_string);

        write!(f, ", batch_prover_url: \"{batch_prover_url}\" }}")?;

        let block_prover_url = self
            .block_prover_url
            .as_ref()
            .map_or_else(|| "None".to_string(), ToString::to_string);

        write!(f, ", block_prover_url: \"{block_prover_url}\" }}")
    }
}

impl Default for BlockProducerConfig {
    fn default() -> Self {
        Self {
            endpoint: Url::parse(
                format!("http://127.0.0.1:{DEFAULT_BLOCK_PRODUCER_PORT}").as_str(),
            )
            .unwrap(),
            store_url: Url::parse(format!("http://127.0.0.1:{DEFAULT_STORE_PORT}").as_str())
                .unwrap(),
            verify_tx_proofs: true,
            batch_prover_url: None,
            block_prover_url: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::BlockProducerConfig;

    #[tokio::test]
    async fn default_block_producer_config() {
        // Default does not panic
        let config = BlockProducerConfig::default();
        // Default can bind
        let socket_addrs = config.endpoint.socket_addrs(|| None).unwrap();
        let socket_addr = socket_addrs.into_iter().next().unwrap();
        let _listener = TcpListener::bind(socket_addr).await.unwrap();
    }
}
