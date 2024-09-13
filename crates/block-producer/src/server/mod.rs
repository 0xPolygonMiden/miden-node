use std::{net::ToSocketAddrs, sync::Arc};

use miden_node_proto::generated::{block_producer::api_server, store::api_client as store_client};
use miden_node_utils::errors::ApiError;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tracing::info;

use crate::{
    batch_builder::{DefaultBatchBuilder, DefaultBatchBuilderOptions},
    block_builder::DefaultBlockBuilder,
    config::BlockProducerConfig,
    state_view::DefaultStateView,
    store::DefaultStore,
    txqueue::{TransactionQueue, TransactionQueueOptions},
    COMPONENT, SERVER_BATCH_SIZE, SERVER_BLOCK_FREQUENCY, SERVER_BUILD_BATCH_FREQUENCY,
    SERVER_MAX_BATCHES_PER_BLOCK,
};

pub mod api;

type Api = api::BlockProducerApi<
    DefaultBatchBuilder<
        DefaultStore,
        DefaultBlockBuilder<DefaultStore, DefaultStateView<DefaultStore>>,
    >,
    DefaultStateView<DefaultStore>,
>;

/// Represents an initialized block-producer component where the RPC connection is open,
/// but not yet actively responding to requests. Separating the connection binding
/// from the server spawning allows the caller to connect other components to the
/// store without resorting to sleeps or other mechanisms to spawn dependent components.
pub struct BlockProducer {
    api_service: api_server::ApiServer<Api>,
    listener: TcpListener,
}

impl BlockProducer {
    /// Performs all expensive initialization tasks, and notably begins listening on the rpc
    /// endpoint without serving the API yet. Incoming requests will be queued until
    /// [`serve`](Self::serve) is called.
    pub async fn init(config: BlockProducerConfig) -> Result<Self, ApiError> {
        info!(target: COMPONENT, %config, "Initializing server");

        let store = Arc::new(DefaultStore::new(
            store_client::ApiClient::connect(config.store_url.to_string())
                .await
                .map_err(|err| ApiError::DatabaseConnectionFailed(err.to_string()))?,
        ));
        let state_view =
            Arc::new(DefaultStateView::new(Arc::clone(&store), config.verify_tx_proofs));

        let block_builder = DefaultBlockBuilder::new(Arc::clone(&store), Arc::clone(&state_view));
        let batch_builder_options = DefaultBatchBuilderOptions {
            block_frequency: SERVER_BLOCK_FREQUENCY,
            max_batches_per_block: SERVER_MAX_BATCHES_PER_BLOCK,
        };
        let batch_builder = Arc::new(DefaultBatchBuilder::new(
            Arc::clone(&store),
            Arc::new(block_builder),
            batch_builder_options,
        ));

        let transaction_queue_options = TransactionQueueOptions {
            build_batch_frequency: SERVER_BUILD_BATCH_FREQUENCY,
            batch_size: SERVER_BATCH_SIZE,
        };
        let queue = Arc::new(TransactionQueue::new(
            state_view,
            Arc::clone(&batch_builder),
            transaction_queue_options,
        ));

        let api_service =
            api_server::ApiServer::new(api::BlockProducerApi::new(Arc::clone(&queue)));

        tokio::spawn(async move { queue.run().await });
        tokio::spawn(async move { batch_builder.run().await });

        let addr = config
            .endpoint
            .to_socket_addrs()
            .map_err(ApiError::EndpointToSocketFailed)?
            .next()
            .ok_or_else(|| ApiError::AddressResolutionFailed(config.endpoint.to_string()))?;

        let listener = TcpListener::bind(addr).await?;

        info!(target: COMPONENT, "Server initialized");

        Ok(Self { api_service, listener })
    }

    /// Serves the block-producers's RPC API.
    ///
    /// Note: this blocks until the server dies.
    pub async fn serve(self) -> Result<(), ApiError> {
        tonic::transport::Server::builder()
            .add_service(self.api_service)
            .serve_with_incoming(TcpListenerStream::new(self.listener))
            .await
            .map_err(ApiError::ApiServeFailed)
    }
}
