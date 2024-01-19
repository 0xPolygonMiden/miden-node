use anyhow::Result;
use miden_crypto::hash::rpo::RpoDigest;
use miden_node_proto::{
    block_producer::api_client as block_producer_client,
    requests::{
        CheckNullifiersRequest, GetBlockHeaderByNumberRequest, SubmitProvenTransactionRequest,
        SyncStateRequest,
    },
    responses::{
        CheckNullifiersResponse, GetBlockHeaderByNumberResponse, SubmitProvenTransactionResponse,
        SyncStateResponse,
    },
    rpc::api_server,
    store::api_client as store_client,
};
use tonic::{
    transport::{Channel, Error},
    Request, Response, Status,
};
use tracing::{info, instrument};

use crate::{config::RpcConfig, COMPONENT};

// RPC API
// ================================================================================================

pub struct RpcApi {
    store: store_client::ApiClient<Channel>,
    block_producer: block_producer_client::ApiClient<Channel>,
}

impl RpcApi {
    pub(super) async fn from_config(config: &RpcConfig) -> Result<Self, Error> {
        let store = store_client::ApiClient::connect(config.store_url.clone()).await?;
        info!(COMPONENT, store_endpoint = config.store_url, "Store client initialized");

        let block_producer =
            block_producer_client::ApiClient::connect(config.block_producer_url.clone()).await?;
        info!(
            COMPONENT,
            block_producer_endpoint = config.block_producer_url,
            "Block producer client initialized",
        );

        Ok(Self {
            store,
            block_producer,
        })
    }
}

#[tonic::async_trait]
impl api_server::Api for RpcApi {
    #[instrument(skip(self), ret, fields(COMPONENT))]
    async fn check_nullifiers(
        &self,
        request: Request<CheckNullifiersRequest>,
    ) -> Result<Response<CheckNullifiersResponse>, Status> {
        // validate all the nullifiers from the user request
        for nullifier in request.get_ref().nullifiers.iter() {
            let _: RpoDigest = nullifier
                .try_into()
                .or(Err(Status::invalid_argument("Digest field is not in the modulos range")))?;
        }

        self.store.clone().check_nullifiers(request).await
    }

    #[instrument(skip(self), ret, fields(COMPONENT))]
    async fn get_block_header_by_number(
        &self,
        request: Request<GetBlockHeaderByNumberRequest>,
    ) -> Result<Response<GetBlockHeaderByNumberResponse>, Status> {
        self.store.clone().get_block_header_by_number(request).await
    }

    #[instrument(skip(self), ret, fields(COMPONENT))]
    async fn sync_state(
        &self,
        request: tonic::Request<SyncStateRequest>,
    ) -> Result<Response<SyncStateResponse>, Status> {
        self.store.clone().sync_state(request).await
    }

    #[instrument(skip(self), ret, fields(COMPONENT))]
    async fn submit_proven_transaction(
        &self,
        request: Request<SubmitProvenTransactionRequest>,
    ) -> Result<tonic::Response<SubmitProvenTransactionResponse>, Status> {
        self.block_producer.clone().submit_proven_transaction(request).await
    }
}
