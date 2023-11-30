use crate::{
    batch_builder::{DefaultBatchBuilder, DefaultBatchBuilderOptions},
    block::Block,
    block_builder::DefaultBlockBuilder,
    config::BlockProducerConfig,
    state_view::DefaulStateView,
    store::{ApplyBlock, ApplyBlockError, BlockInputsError, Store, TxInputs, TxInputsError},
    txqueue::{DefaultTransactionQueue, DefaultTransactionQueueOptions, TransactionQueue},
    SharedProvenTx,
};
use anyhow::Result;
use async_trait::async_trait;
use miden_node_proto::{
    block_producer::api_server, domain::BlockInputs, requests::SubmitProvenTransactionRequest,
    responses::SubmitProvenTransactionResponse,
};
use miden_objects::{accounts::AccountId, Digest};
use std::{net::ToSocketAddrs, sync::Arc, time::Duration};
use tonic::{transport::Server, Status};
use tracing::info;

struct RpcStore {}

#[async_trait]
impl ApplyBlock for RpcStore {
    async fn apply_block(
        &self,
        _block: Arc<Block>,
    ) -> Result<(), ApplyBlockError> {
        todo!()
    }
}

#[async_trait]
impl Store for RpcStore {
    async fn get_tx_inputs(
        &self,
        _proven_tx: SharedProvenTx,
    ) -> Result<TxInputs, TxInputsError> {
        todo!()
    }

    async fn get_block_inputs(
        &self,
        _updated_accounts: impl Iterator<Item = &AccountId> + Send,
        _produced_nullifiers: impl Iterator<Item = &Digest> + Send,
    ) -> Result<BlockInputs, BlockInputsError> {
        todo!()
    }
}

pub struct BlockProducerApi<T> {
    queue: Arc<T>,
}

#[tonic::async_trait]
impl<T> api_server::Api for BlockProducerApi<T>
where
    T: TransactionQueue,
{
    async fn submit_proven_transaction(
        &self,
        _request: tonic::Request<SubmitProvenTransactionRequest>,
    ) -> Result<tonic::Response<SubmitProvenTransactionResponse>, Status> {
        todo!()
    }
}

pub async fn serve(config: BlockProducerConfig) -> Result<()> {
    let host_port = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = host_port.to_socket_addrs()?.collect();

    let store = Arc::new(RpcStore {});
    let block_builder = DefaultBlockBuilder::new(store.clone());
    let batch_builder_options = DefaultBatchBuilderOptions {
        block_frequency: Duration::from_secs(10),
        max_batches_per_block: 4,
    };
    let batch_builder = DefaultBatchBuilder::new(Arc::new(block_builder), batch_builder_options);
    let state_view = DefaulStateView::new(store.clone());

    let transaction_queue_options = DefaultTransactionQueueOptions {
        build_batch_frequency: Duration::from_secs(2),
        batch_size: 2,
    };
    let queue = Arc::new(DefaultTransactionQueue::new(
        Arc::new(state_view),
        Arc::new(batch_builder),
        transaction_queue_options,
    ));

    let block_producer = api_server::ApiServer::new(BlockProducerApi {
        queue: queue.clone(),
    });

    info!(host = config.endpoint.host, port = config.endpoint.port, "Server initialized",);

    tokio::spawn(async move {
        info!("Block producer task created");
        queue.run().await
    });
    Server::builder().add_service(block_producer).serve(addrs[0]).await?;

    Ok(())
}
