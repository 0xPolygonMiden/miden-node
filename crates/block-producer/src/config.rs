use std::fmt::{Display, Formatter};

use miden_node_utils::config::{
    DEFAULT_BATCH_PROVER_PORT, DEFAULT_BLOCK_PRODUCER_PORT, DEFAULT_STORE_PORT,
};
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
    pub remote_batch_prover: Url,
}

impl Display for BlockProducerConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ endpoint: \"{}\", store_url: \"{}\", remote_batch_prover: \"{}\" }}",
            self.endpoint, self.store_url, self.remote_batch_prover,
        ))
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
            remote_batch_prover: Url::parse(
                format!("http://127.0.0.1:{DEFAULT_BATCH_PROVER_PORT}").as_str(),
            )
            .unwrap(),
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
