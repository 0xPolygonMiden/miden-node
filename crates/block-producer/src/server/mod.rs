use std::{net::ToSocketAddrs, sync::Arc};

use miden_node_proto::generated::{block_producer::api_server, store::api_client as store_client};
use miden_node_utils::errors::ApiError;
use tonic::transport::Server;
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

// BLOCK PRODUCER INITIALIZER
// ================================================================================================

pub async fn serve(config: BlockProducerConfig) -> Result<(), ApiError> {
    info!(target: COMPONENT, %config, "Initializing server");

    let store = Arc::new(DefaultStore::new(
        store_client::ApiClient::connect(config.store_url.to_string())
            .await
            .map_err(|err| ApiError::DatabaseConnectionFailed(err.to_string()))?,
    ));
    let state_view = Arc::new(DefaultStateView::new(store.clone(), config.verify_tx_proofs));

    let block_builder = DefaultBlockBuilder::new(store.clone(), state_view.clone());
    let batch_builder_options = DefaultBatchBuilderOptions {
        block_frequency: SERVER_BLOCK_FREQUENCY,
        max_batches_per_block: SERVER_MAX_BATCHES_PER_BLOCK,
    };
    let batch_builder =
        Arc::new(DefaultBatchBuilder::new(Arc::new(block_builder), batch_builder_options));

    let transaction_queue_options = TransactionQueueOptions {
        build_batch_frequency: SERVER_BUILD_BATCH_FREQUENCY,
        batch_size: SERVER_BATCH_SIZE,
    };
    let queue = Arc::new(TransactionQueue::new(
        state_view,
        batch_builder.clone(),
        transaction_queue_options,
    ));

    let block_producer = api_server::ApiServer::new(api::BlockProducerApi::new(queue.clone()));

    tokio::spawn(async move { queue.run().await });
    tokio::spawn(async move { batch_builder.run().await });

    info!(target: COMPONENT, "Server initialized");

    let addr = config
        .endpoint
        .to_socket_addrs()
        .map_err(ApiError::EndpointToSocketFailed)?
        .next()
        .ok_or_else(|| ApiError::AddressResolutionFailed(config.endpoint.to_string()))?;

    Server::builder()
        .add_service(block_producer)
        .serve(addr)
        .await
        .map_err(ApiError::ApiServeFailed)?;

    Ok(())
}
