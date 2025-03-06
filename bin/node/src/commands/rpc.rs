use anyhow::Context;
use miden_node_rpc::server::Rpc;
use miden_node_utils::grpc::UrlExt;
use url::Url;

use super::{ENV_BLOCK_PRODUCER_URL, ENV_RPC_URL, ENV_STORE_URL};

#[derive(clap::Subcommand)]
pub enum RpcCommand {
    /// Starts the RPC component.
    Start(RpcConfig),
}

#[derive(clap::Args)]
pub struct RpcConfig {
    /// Url at which to serve the gRPC API.
    #[arg(long = "rpc.url", env = ENV_RPC_URL)]
    url: Url,

    /// The store's gRPC url.
    #[arg(long = "store.url", env = ENV_STORE_URL)]
    store_url: Url,

    /// The block-producer's gRPC url.
    #[arg(long = "block-producer.url", env = ENV_BLOCK_PRODUCER_URL)]
    block_producer_url: Url,
}

impl RpcConfig {
    /// Initializes and runs the Miden node's [`Rpc`] component.
    pub async fn run(self) -> anyhow::Result<()> {
        let store_url = self
            .store_url
            .to_socket()
            .context("Failed to extract socket address from store URL")?;
        let block_producer_url = self
            .block_producer_url
            .to_socket()
            .context("Failed to extract socket address from store URL")?;

        let listener =
            self.url.to_socket().context("Failed to extract socket address from RPC URL")?;
        let listener = tokio::net::TcpListener::bind(listener)
            .await
            .context("Failed to bind to RPC's gRPC URL")?;

        Rpc::init(listener, store_url, block_producer_url)
            .await
            .context("Loading RPC")?
            .serve()
            .await
            .context("Serving RPC")
    }
}
