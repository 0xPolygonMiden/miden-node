use url::Url;

use super::{ENV_BLOCK_PRODUCER_URL, ENV_STORE_URL};

#[derive(clap::Subcommand)]
pub enum BlockProducerCommand {
    /// Starts the block-producer component.
    Start {
        /// Url at which to serve the gRPC API.
        #[arg(env = ENV_BLOCK_PRODUCER_URL)]
        url: Url,

        /// The store's gRPC url.
        #[arg(long = "store.url", env = ENV_STORE_URL)]
        store_url: Url,
    },
}
