use std::path::PathBuf;

use anyhow::Context;
use miden_node_store::server::Store;
use miden_node_utils::grpc::UrlExt;
use url::Url;

use super::{ENV_STORE_DIRECTORY, ENV_STORE_URL};

#[derive(clap::Subcommand)]
pub enum StoreCommand {
    Init,
    /// Starts the store component.
    Start(StoreConfig),
}

#[derive(clap::Args)]
pub struct StoreConfig {
    /// Url at which to serve the gRPC API.
    #[arg(env = ENV_STORE_URL)]
    url: Url,

    /// Directory in which to store the database and raw block data.
    #[arg(env = ENV_STORE_DIRECTORY)]
    data_directory: PathBuf,
}

impl StoreConfig {
    /// Initializes and runs the Miden node's [`Store`] component.
    pub async fn run(self) -> anyhow::Result<()> {
        let listener = self
            .url
            .to_socket()
            .context("Failed to extract socket address from store URL")?;
        let listener = tokio::net::TcpListener::bind(listener)
            .await
            .context("Failed to bind to store's gRPC URL")?;

        Store::init(listener, self.data_directory)
            .await
            .context("Loading store")?
            .serve()
            .await
            .context("Serving store")
    }
}
