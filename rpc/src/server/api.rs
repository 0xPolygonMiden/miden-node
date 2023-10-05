use crate::config::RpcConfig;
use anyhow::Result;
use miden_crypto::hash::rpo::RpoDigest;
use miden_node_proto::{
    rpc::{
        api_server, CheckNullifiersRequest, CheckNullifiersResponse,
        FetchBlockHeaderByNumberRequest, FetchBlockHeaderByNumberResponse,
    },
    store::{self, api_client},
};
use std::net::ToSocketAddrs;
use tonic::{
    transport::{Channel, Error, Server},
    Request, Response, Status,
};
use tracing::info;

pub struct RpcApi {
    store: api_client::ApiClient<Channel>,
}

impl RpcApi {
    async fn from_config(config: &RpcConfig) -> Result<Self, Error> {
        let client = api_client::ApiClient::connect(config.store.clone()).await?;
        info!(store = config.store, "Store client initialized",);
        Ok(Self { store: client })
    }
}

#[tonic::async_trait]
impl api_server::Api for RpcApi {
    async fn check_nullifiers(
        &self,
        request: Request<CheckNullifiersRequest>,
    ) -> Result<Response<CheckNullifiersResponse>, Status> {
        let user_request = request.into_inner();

        // validate all the nullifiers from the user request
        for nullifier in user_request.nullifiers.iter() {
            let _: RpoDigest = nullifier
                .try_into()
                .or(Err(Status::invalid_argument("Digest field is not in the modulos range")))?;
        }

        let store_response = self
            .store
            .clone()
            .check_nullifiers(Request::new(store::CheckNullifiersRequest {
                nullifiers: user_request.nullifiers,
            }))
            .await?
            .into_inner();

        Ok(Response::new(CheckNullifiersResponse {
            proofs: store_response.proofs,
        }))
    }

    async fn fetch_block_header_by_number(
        &self,
        request: Request<FetchBlockHeaderByNumberRequest>,
    ) -> Result<Response<FetchBlockHeaderByNumberResponse>, Status> {
        let user_request = request.into_inner();

        let store_response = self
            .store
            .clone()
            .fetch_block_header_by_number(Request::new(store::FetchBlockHeaderByNumberRequest {
                block_num: user_request.block_num,
            }))
            .await?
            .into_inner();

        Ok(Response::new(FetchBlockHeaderByNumberResponse {
            block_header: store_response.block_header,
        }))
    }
}

pub async fn serve(config: RpcConfig) -> Result<()> {
    let host_port = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = host_port.to_socket_addrs()?.collect();

    let api = RpcApi::from_config(&config).await?;
    let rpc = api_server::ApiServer::new(api);

    info!(host = config.endpoint.host, port = config.endpoint.port, "Server initialized");

    Server::builder().add_service(rpc).serve(addrs[0]).await?;

    Ok(())
}
