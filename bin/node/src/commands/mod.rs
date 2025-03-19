pub mod block_producer;
pub mod bundled;
pub mod rpc;
pub mod store;

const ENV_BLOCK_PRODUCER_URL: &str = "MIDEN_NODE_BLOCK_PRODUCER_URL";
const ENV_BATCH_PROVER_URL: &str = "MIDEN_NODE_BATCH_PROVER_URL";
const ENV_BLOCK_PROVER_URL: &str = "MIDEN_NODE_BLOCK_PROVER_URL";
const ENV_RPC_URL: &str = "MIDEN_NODE_RPC_URL";
const ENV_STORE_URL: &str = "MIDEN_NODE_STORE_URL";
const ENV_DATA_DIRECTORY: &str = "MIDEN_NODE_DATA_DIRECTORY";
const ENV_ENABLE_OTEL: &str = "MIDEN_NODE_ENABLE_OTEL";

const DEFAULT_BLOCK_INTERVAL: &str = "5s";
const DEFAULT_BATCH_INTERVAL: &str = "2s";
