mod genesis;
pub mod init;
pub mod start;
pub use genesis::make_genesis;

pub mod block_producer;
pub mod node;
pub mod rpc;
pub mod store;

const ENV_BLOCK_PRODUCER_URL: &'static str = "MIDEN_NODE_BLOCK_PRODUCER_URL";
const ENV_RPC_URL: &'static str = "MIDEN_NODE_RPC_URL";
const ENV_STORE_URL: &'static str = "MIDEN_NODE_STORE_URL";
const ENV_STORE_DIRECTORY: &'static str = "MIDEN_NODE_STORE_DATA_DIRECTORY";
