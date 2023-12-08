use std::{net::ToSocketAddrs, sync::Arc, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use miden_crypto::utils::Deserializable;
use miden_node_proto::{
    account_id,
    block_producer::api_server,
    digest,
    domain::BlockInputs,
    requests::{
        AccountUpdate, ApplyBlockRequest, GetBlockInputsRequest, GetTransactionInputsRequest,
        NoteCreated, SubmitProvenTransactionRequest,
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
    state_view::DefaulStateView,
    store::{ApplyBlock, ApplyBlockError, BlockInputsError, Store, TxInputs, TxInputsError},
    txqueue::{DefaultTransactionQueue, DefaultTransactionQueueOptions, TransactionQueue},
    SharedProvenTx,
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
            accounts: block
                .updated_accounts
                .iter()
                .map(|(account_id, account_hash)| AccountUpdate {
                    account_id: Some((*account_id).into()),
                    account_hash: Some(account_hash.into()),
                })
                .collect(),
            nullifiers: block
                .produced_nullifiers
                .iter()
                .map(|nullifier| nullifier.into())
                .collect(),
            notes: block
                .created_notes
                .iter()
                .map(|(note_idx, note)| NoteCreated {
                    note_hash: Some(note.note_hash().into()),
                    sender: note.metadata().sender().into(),
                    tag: note.metadata().tag().into(),
                    num_assets: u64::from(note.metadata().num_assets()) as u32,
                    note_index: *note_idx as u32,
                })
                .collect(),
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
                .map(|note| note.nullifier().into())
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
                .get(0)
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
