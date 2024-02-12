use anyhow::Result;
use miden_node_proto::generated::{
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
use miden_objects::Digest;
use tonic::{
    transport::{Channel, Error},
    Request, Response, Status,
};
use tracing::{debug, info, instrument};

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
        info!(target: COMPONENT, store_endpoint = config.store_url, "Store client initialized");

        let block_producer =
            block_producer_client::ApiClient::connect(config.block_producer_url.clone()).await?;
        info!(
            target: COMPONENT,
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
    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(
        target = "miden-rpc",
        name = "rpc:check_nullifiers",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn check_nullifiers(
        &self,
        request: Request<CheckNullifiersRequest>,
    ) -> Result<Response<CheckNullifiersResponse>, Status> {
        debug!(target: COMPONENT, request = ?request.get_ref());

        // validate all the nullifiers from the user request
        for nullifier in request.get_ref().nullifiers.iter() {
            let _: Digest = nullifier
                .try_into()
                .or(Err(Status::invalid_argument("Digest field is not in the modulus range")))?;
        }

        self.store.clone().check_nullifiers(request).await
    }

    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(
        target = "miden-rpc",
        name = "rpc:get_block_header_by_number",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_block_header_by_number(
        &self,
        request: Request<GetBlockHeaderByNumberRequest>,
    ) -> Result<Response<GetBlockHeaderByNumberResponse>, Status> {
        info!(target: COMPONENT, request = ?request.get_ref());

        self.store.clone().get_block_header_by_number(request).await
    }

    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(
        target = "miden-rpc",
        name = "rpc:sync_state",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn sync_state(
        &self,
        request: Request<SyncStateRequest>,
    ) -> Result<Response<SyncStateResponse>, Status> {
        debug!(target: COMPONENT, request = ?request.get_ref());

        self.store.clone().sync_state(request).await
    }

    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(target = "miden-rpc", name = "rpc:submit_proven_transaction", skip_all, err)]
    async fn submit_proven_transaction(
        &self,
        request: Request<SubmitProvenTransactionRequest>,
    ) -> Result<Response<SubmitProvenTransactionResponse>, Status> {
        debug!(target: COMPONENT, request = ?request.get_ref());

        self.block_producer.clone().submit_proven_transaction(request).await
    }
}
