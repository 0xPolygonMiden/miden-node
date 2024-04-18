use std::net::ToSocketAddrs;

use miden_node_proto::generated::rpc::api_server;
use miden_node_utils::errors::ApiError;
use tonic::transport::Server;
use tracing::info;

use crate::{config::RpcConfig, COMPONENT};

mod api;

// RPC INITIALIZER
// ================================================================================================

pub async fn serve(config: RpcConfig) -> Result<(), ApiError> {
    info!(target: COMPONENT, %config, "Initializing server");

    let api = api::RpcApi::from_config(&config)
        .await
        .map_err(ApiError::ApiInitialisationFailed)?;
    let rpc = api_server::ApiServer::new(api);

    info!(target: COMPONENT, "Server initialized");

    let addr = config
        .endpoint
        .to_socket_addrs()
        .map_err(ApiError::EndpointToSocketFailed)?
        .next()
        .ok_or(config.endpoint.to_string())
        .map_err(ApiError::AddressResolutionFailed)?;

    Server::builder()
        .add_service(rpc)
        .serve(addr)
        .await
        .map_err(ApiError::ApiServeFailed)?;

    Ok(())
}
