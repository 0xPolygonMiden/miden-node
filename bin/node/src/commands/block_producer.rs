use anyhow::Context;
use miden_node_block_producer::server::BlockProducer;
use miden_node_utils::grpc::UrlExt;
use url::Url;

use super::{
    ENV_BATCH_PROVER_URL, ENV_BLOCK_PRODUCER_URL, ENV_BLOCK_PROVER_URL, ENV_ENABLE_OTEL,
    ENV_STORE_URL,
};

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

        /// The remote batch prover's gRPC url. If unset, will default to running a prover
        /// in-process which is expensive.
        #[arg(long = "batch_prover.url", env = ENV_BATCH_PROVER_URL)]
        batch_prover_url: Option<Url>,

        /// The remote block prover's gRPC url. If unset, will default to running a prover
        /// in-process which is expensive.
        #[arg(long = "block_prover.url", env = ENV_BLOCK_PROVER_URL)]
        block_prover_url: Option<Url>,

        /// Enables the exporting of traces for OpenTelemetry.
        ///
        /// This can be further configured using environment variables as defined in the official
        /// OpenTelemetry documentation. See our operator manual for further details.
        #[arg(long = "open-telemetry", default_value_t = false, env = ENV_ENABLE_OTEL)]
        open_telemetry: bool,
    },
}

impl BlockProducerCommand {
    pub async fn handle(self) -> anyhow::Result<()> {
        let Self::Start {
            url,
            store_url,
            batch_prover_url,
            block_prover_url,
            // Note: open-telemetry is handled in main.
            open_telemetry: _,
        } = self;

        let store_url = store_url
            .to_socket()
            .context("Failed to extract socket address from store URL")?;

        let listener =
            url.to_socket().context("Failed to extract socket address from store URL")?;
        let listener = tokio::net::TcpListener::bind(listener)
            .await
            .context("Failed to bind to store's gRPC URL")?;

        BlockProducer::init(listener, store_url, batch_prover_url, block_prover_url)
            .await
            .context("Loading store")?
            .serve()
            .await
            .context("Serving store")
    }

    pub fn is_open_telemetry_enabled(&self) -> bool {
        let Self::Start { open_telemetry, .. } = self;
        *open_telemetry
    }
}
