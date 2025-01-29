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
}

impl Display for BlockProducerConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ endpoint: \"{}\", store_url: \"{}\" }}",
            self.endpoint, self.store_url
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BlockProducerConfig;

    #[test]
    fn default_block_producer_config() {
        let _config = BlockProducerConfig::default();
    }
}
