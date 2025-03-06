use url::Url;

use super::{ENV_BLOCK_PRODUCER_URL, ENV_RPC_URL, ENV_STORE_URL};

#[derive(clap::Subcommand)]
pub enum RpcCommand {
    /// Starts the RPC component.
    Start {
        /// Url at which to serve the gRPC API.
        #[arg(long = "rpc.url", env = ENV_RPC_URL)]
        url: Url,

        /// The store's gRPC url.
        #[arg(long = "store.url", env = ENV_STORE_URL)]
        store_url: Url,

        /// The block-producer's gRPC url.
        #[arg(long = "block-producer.url", env = ENV_BLOCK_PRODUCER_URL)]
        block_producer_url: Url,
    },
}
