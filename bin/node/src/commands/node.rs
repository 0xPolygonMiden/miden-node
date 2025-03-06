use std::{collections::HashMap, path::PathBuf};

use anyhow::Context;
use miden_node_block_producer::server::BlockProducer;
use miden_node_rpc::server::Rpc;
use miden_node_store::server::Store;
use miden_node_utils::grpc::UrlExt;
use tokio::{net::TcpListener, task::JoinSet};
use url::Url;

use super::{ENV_RPC_URL, ENV_STORE_DIRECTORY};

#[derive(clap::Subcommand)]
pub enum NodeCommand {
    /// Runs all three node components in the same process.
    ///
    /// The gRPC endpoints for the store and block-producer will each be assigned a random open
    /// port on localhost (127.0.0.1:0).
    Start(NodeConfig),
}

#[derive(clap::Args)]
pub struct NodeConfig {
    /// Url at which to serve the RPC component's gRPC API.
    #[arg(long = "rpc.url", env = ENV_RPC_URL)]
    rpc_url: Url,

    /// Directory in which the Store component should store the database and raw block data.
    #[arg(long = "store.data-directory", env = ENV_STORE_DIRECTORY)]
    store_data_directory: PathBuf,
}

impl NodeConfig {
    pub async fn run(self) -> anyhow::Result<()> {
        // Start listening on all gRPC urls so that inter-component connections can be created
        // before each component is fully started up.
        //
        // This is required because `tonic` does not handle retries nor reconnections and our
        // services expect to be able to connect on startup.
        let grpc_rpc = self.rpc_url.to_socket().context("Failed to to RPC gRPC socket")?;
        let grpc_rpc = TcpListener::bind(grpc_rpc)
            .await
            .context("Failed to bind to RPC gRPC endpoint")?;
        let grpc_store = TcpListener::bind("127.0.0.1:0")
            .await
            .context("Failed to bind to store gRPC endpoint")?;
        let grpc_block_producer = TcpListener::bind("127.0.0.1:0")
            .await
            .context("Failed to bind to block-producer gRPC endpoint")?;

        let store_address =
            grpc_store.local_addr().context("Failed to retrieve the store's gRPC address")?;
        let block_producer_address = grpc_block_producer
            .local_addr()
            .context("Failed to retrieve the block-producer's gRPC address")?;

        let mut join_set = JoinSet::new();

        // Start store. The store endpoint is available after loading completes.
        let store = Store::init(grpc_store, self.store_data_directory)
            .await
            .context("Loading store")?;
        let store_id =
            join_set.spawn(async move { store.serve().await.context("Serving store") }).id();

        // Start block-producer. The block-producer's endpoint is available after loading completes.
        let block_producer = BlockProducer::init(grpc_block_producer, store_address)
            .await
            .context("Loading block-producer")?;
        let block_producer_id = join_set
            .spawn(async move { block_producer.serve().await.context("Serving block-producer") })
            .id();

        // Start RPC component.
        let rpc = Rpc::init(grpc_rpc, store_address, block_producer_address)
            .await
            .context("Loading RPC")?;
        let rpc_id = join_set.spawn(async move { rpc.serve().await.context("Serving RPC") }).id();

        // Lookup table so we can identify the failed component.
        let component_ids = HashMap::from([
            (store_id, "store"),
            (block_producer_id, "block-producer"),
            (rpc_id, "rpc"),
        ]);

        // SAFETY: The joinset is definitely not empty.
        let component_result = join_set.join_next_with_id().await.unwrap();

        // We expect components to run indefinitely, so we treat any return as fatal.
        //
        // Map all outcomes to an error, and provide component context.
        let (id, err) = match component_result {
            Ok((id, Ok(_))) => (id, Err(anyhow::anyhow!("Component completed unexpectedly"))),
            Ok((id, Err(err))) => (id, Err(err)),
            Err(join_err) => (join_err.id(), Err(join_err).context("Joining component task")),
        };
        let component = component_ids.get(&id).unwrap_or(&"unknown");

        // We could abort and gracefully shutdown the other components, but since we're crashing the
        // node there is no point.

        err.context(format!("Component {component} failed"))
    }
}
