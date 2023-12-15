use std::{net::ToSocketAddrs, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use miden_crypto::utils::Deserializable;
use miden_node_proto::{
    account_id,
    block_producer::api_server,
    conversion::convert,
    digest,
    domain::BlockInputs,
    requests::{
        ApplyBlockRequest, GetBlockInputsRequest, GetTransactionInputsRequest,
        SubmitProvenTransactionRequest,
    },
    responses::SubmitProvenTransactionResponse,
    store::api_client as store_client,
};
use miden_objects::{accounts::AccountId, transaction::ProvenTransaction, Digest};
use tonic::{
    transport::{Channel, Server},
    Status,
};
use tracing::info;

use crate::{
    batch_builder::{DefaultBatchBuilder, DefaultBatchBuilderOptions},
    block::Block,
    block_builder::DefaultBlockBuilder,
    config::BlockProducerConfig,
    state_view::DefaultStateView,
    store::{ApplyBlock, ApplyBlockError, BlockInputsError, Store, TxInputs, TxInputsError},
    txqueue::{DefaultTransactionQueue, DefaultTransactionQueueOptions, TransactionQueue},
    SharedProvenTx, COMPONENT, SERVER_BATCH_SIZE, SERVER_BLOCK_FREQUENCY,
    SERVER_BUILD_BATCH_FREQUENCY, SERVER_MAX_BATCHES_PER_BLOCK,
};

struct DefaultStore {
    store: store_client::ApiClient<Channel>,
}

#[async_trait]
impl ApplyBlock for DefaultStore {
    async fn apply_block(
        &self,
        block: Arc<Block>,
    ) -> Result<(), ApplyBlockError> {
        let request = tonic::Request::new(ApplyBlockRequest {
            block: Some(block.header.into()),
            accounts: convert(block.updated_accounts.clone()),
            nullifiers: convert(block.produced_nullifiers.clone()),
            notes: convert(block.created_notes.clone()),
        });

        let _ = self
            .store
            .clone()
            .apply_block(request)
            .await
            .map_err(|status| ApplyBlockError::GrpcClientError(status.message().to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl Store for DefaultStore {
    async fn get_tx_inputs(
        &self,
        proven_tx: SharedProvenTx,
    ) -> Result<TxInputs, TxInputsError> {
        let request = tonic::Request::new(GetTransactionInputsRequest {
            account_ids: vec![proven_tx.account_id().into()],
            nullifiers: proven_tx
                .consumed_notes()
                .iter()
                .map(|nullifier| nullifier.inner().into())
                .collect(),
        });
        let response = self
            .store
            .clone()
            .get_transaction_inputs(request)
            .await
            .map_err(|status| TxInputsError::GrpcClientError(status.message().to_string()))?
            .into_inner();

        let account_hash = {
            let account_state = response
                .account_states
                .first()
                .ok_or(TxInputsError::MalformedResponse("account_states empty".to_string()))?;

            let account_id_from_store: AccountId = account_state
                .account_id
                .clone()
                .ok_or(TxInputsError::MalformedResponse("empty account id".to_string()))?
                .try_into()?;

            if account_id_from_store != proven_tx.account_id() {
                return Err(TxInputsError::MalformedResponse(format!(
                    "incorrect account id returned from store. Got: {}, expected: {}",
                    account_id_from_store,
                    proven_tx.account_id()
                )));
            }

            account_state.account_hash.clone().map(Digest::try_from).transpose()?
        };

        let nullifiers = {
            let mut nullifiers = Vec::new();

            for nullifier_record in response.nullifiers {
                let nullifier = nullifier_record
                    .nullifier
                    .ok_or(TxInputsError::MalformedResponse(
                        "nullifier record contains empty nullifier".to_string(),
                    ))?
                    .try_into()?;

                // `block_num` is nonzero if already consumed; 0 otherwise
                nullifiers.push((nullifier, nullifier_record.block_num != 0))
            }

            nullifiers.into_iter().collect()
        };

        Ok(TxInputs {
            account_hash,
            nullifiers,
        })
    }

    async fn get_block_inputs(
        &self,
        updated_accounts: impl Iterator<Item = &AccountId> + Send,
        produced_nullifiers: impl Iterator<Item = &Digest> + Send,
    ) -> Result<BlockInputs, BlockInputsError> {
        let request = tonic::Request::new(GetBlockInputsRequest {
            account_ids: updated_accounts
                .map(|&account_id| account_id::AccountId::from(account_id))
                .collect(),
            nullifiers: produced_nullifiers.map(digest::Digest::from).collect(),
        });

        let store_response = self
            .store
            .clone()
            .get_block_inputs(request)
            .await
            .map_err(|err| BlockInputsError::GrpcClientError(err.message().to_string()))?
            .into_inner();

        Ok(store_response.try_into()?)
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
        request: tonic::Request<SubmitProvenTransactionRequest>,
    ) -> Result<tonic::Response<SubmitProvenTransactionResponse>, Status> {
        let request = request.into_inner();

        let tx = ProvenTransaction::read_from_bytes(&request.transaction)
            .map_err(|_| Status::invalid_argument("Invalid transaction"))?;

        self.queue
            .add_transaction(Arc::new(tx))
            .await
            .map_err(|err| Status::invalid_argument(format!("{:?}", err)))?;

        Ok(tonic::Response::new(SubmitProvenTransactionResponse {}))
    }
}

pub async fn serve(config: BlockProducerConfig) -> Result<()> {
    let host_port = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = host_port.to_socket_addrs()?.collect();

    let store = Arc::new(DefaultStore {
        store: store_client::ApiClient::connect(config.store_endpoint.to_string()).await?,
    });
    let block_builder = DefaultBlockBuilder::new(store.clone());
    let batch_builder_options = DefaultBatchBuilderOptions {
        block_frequency: SERVER_BLOCK_FREQUENCY,
        max_batches_per_block: SERVER_MAX_BATCHES_PER_BLOCK,
    };
    let batch_builder =
        Arc::new(DefaultBatchBuilder::new(Arc::new(block_builder), batch_builder_options));
    let state_view = DefaultStateView::new(store.clone());

    let transaction_queue_options = DefaultTransactionQueueOptions {
        build_batch_frequency: SERVER_BUILD_BATCH_FREQUENCY,
        batch_size: SERVER_BATCH_SIZE,
    };
    let queue = Arc::new(DefaultTransactionQueue::new(
        Arc::new(state_view),
        batch_builder.clone(),
        transaction_queue_options,
    ));

    let block_producer = api_server::ApiServer::new(BlockProducerApi {
        queue: queue.clone(),
    });

    tokio::spawn(async move {
        info!(COMPONENT, "transaction queue started");
        queue.run().await
    });

    tokio::spawn(async move {
        info!(COMPONENT, "batch builder started");
        batch_builder.run().await
    });

    info!(
        COMPONENT,
        host = config.endpoint.host,
        port = config.endpoint.port,
        "Server initialized",
    );
    Server::builder().add_service(block_producer).serve(addrs[0]).await?;

    Ok(())
}
