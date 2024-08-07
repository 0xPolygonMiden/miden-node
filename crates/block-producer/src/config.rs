use std::fmt::{Display, Formatter};

use miden_node_utils::config::{Endpoint, DEFAULT_BLOCK_PRODUCER_PORT, DEFAULT_STORE_PORT};
use serde::{Deserialize, Serialize};

// Main config
// ================================================================================================

/// Block producer specific configuration
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockProducerConfig {
    pub endpoint: Endpoint,

    /// Store gRPC endpoint in the format `http://<host>[:<port>]`.
    pub store_url: String,

    /// Enable or disable the verification of transaction proofs before they are accepted into the
    /// transaction queue.
    ///
    /// Disabling transaction proof verification will speed up transaction processing as proof
    /// verification may take ~15ms/proof. This is OK when all transactions are forwarded to the
    /// block producer from the RPC component as transaction proofs are also verified there.
    pub verify_tx_proofs: bool,
}

impl BlockProducerConfig {
    pub fn endpoint_url(&self) -> String {
        self.endpoint.to_string()
    }
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
            endpoint: Endpoint::localhost(DEFAULT_BLOCK_PRODUCER_PORT),
            store_url: Endpoint::localhost(DEFAULT_STORE_PORT).to_string(),
            verify_tx_proofs: true,
        }
    }
}
