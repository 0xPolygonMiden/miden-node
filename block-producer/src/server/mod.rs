use std::{net::ToSocketAddrs, sync::Arc};

use anyhow::{anyhow, Result};
use miden_node_proto::{block_producer::api_server, store::api_client as store_client};
use tonic::transport::Server;
use tracing::{info, info_span, instrument, Instrument};

use crate::{
    batch_builder::{DefaultBatchBuilder, DefaultBatchBuilderOptions},
    block_builder::DefaultBlockBuilder,
    config::BlockProducerConfig,
    state_view::DefaultStateView,
    store::DefaultStore,
    txqueue::{DefaultTransactionQueue, DefaultTransactionQueueOptions},
    COMPONENT, SERVER_BATCH_SIZE, SERVER_BLOCK_FREQUENCY, SERVER_BUILD_BATCH_FREQUENCY,
    SERVER_MAX_BATCHES_PER_BLOCK,
};

// TODO: does this need to be public?
pub mod api;

// BLOCK PRODUCER INITIALIZER
// ================================================================================================

/// TODO: add comments
#[instrument(target = "miden-block-producer", skip_all)]
pub async fn serve(config: BlockProducerConfig) -> Result<()> {
    info!(target: COMPONENT, %config, "Initializing server");

    let store = Arc::new(DefaultStore::new(
        store_client::ApiClient::connect(config.store_url.to_string()).await?,
    ));
    let state_view = Arc::new(DefaultStateView::new(store.clone()));

    let block_builder = DefaultBlockBuilder::new(store.clone(), state_view.clone());
    let batch_builder_options = DefaultBatchBuilderOptions {
        block_frequency: SERVER_BLOCK_FREQUENCY,
        max_batches_per_block: SERVER_MAX_BATCHES_PER_BLOCK,
    };
    let batch_builder =
        Arc::new(DefaultBatchBuilder::new(Arc::new(block_builder), batch_builder_options));

    let transaction_queue_options = DefaultTransactionQueueOptions {
        build_batch_frequency: SERVER_BUILD_BATCH_FREQUENCY,
        batch_size: SERVER_BATCH_SIZE,
    };
    let queue = Arc::new(DefaultTransactionQueue::new(
        state_view,
        batch_builder.clone(),
        transaction_queue_options,
    ));

    let block_producer = api_server::ApiServer::new(api::BlockProducerApi::new(queue.clone()));

    tokio::spawn(async move {
        queue
            .run()
            .instrument(info_span!(target: COMPONENT, "transaction_queue_start"))
            .await
    });

    tokio::spawn(async move {
        batch_builder
            .run()
            .instrument(info_span!(target: COMPONENT, "batch_builder_start"))
            .await
    });

    info!(target: COMPONENT, "Server initialized");

    let addr = config
        .endpoint
        .to_socket_addrs()?
        .next()
        .ok_or(anyhow!("Couldn't resolve server address"))?;
    Server::builder().add_service(block_producer).serve(addr).await?;

    Ok(())
}
