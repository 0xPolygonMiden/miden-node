use anyhow::Context;
use miden_node_block_producer::server::BlockProducer;
use miden_node_utils::grpc::UrlExt;
use url::Url;

use super::{ENV_BLOCK_PRODUCER_URL, ENV_STORE_URL};

#[derive(clap::Subcommand)]
pub enum BlockProducerCommand {
    /// Starts the block-producer component.
    Start(BlockProducerConfig),
}

#[derive(clap::Args)]
pub struct BlockProducerConfig {
    /// Url at which to serve the gRPC API.
    #[arg(env = ENV_BLOCK_PRODUCER_URL)]
    url: Url,

    /// The store's gRPC url.
    #[arg(long = "store.url", env = ENV_STORE_URL)]
    store_url: Url,
}

impl BlockProducerConfig {
    /// Initializes and runs the Miden node's [`BlockProducer`] component.
    pub async fn run(self) -> anyhow::Result<()> {
        let store_url = self
            .store_url
            .to_socket()
            .context("Failed to extract socket address from store URL")?;

        let listener = self
            .url
            .to_socket()
            .context("Failed to extract socket address from store URL")?;
        let listener = tokio::net::TcpListener::bind(listener)
            .await
            .context("Failed to bind to store's gRPC URL")?;

        BlockProducer::init(listener, store_url)
            .await
            .context("Loading store")?
            .serve()
            .await
            .context("Serving store")
    }
}
