use std::net::ToSocketAddrs;

use api::RpcApi;
use miden_node_proto::generated::rpc::api_server;
use miden_node_utils::errors::ApiError;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tracing::info;

use crate::{config::RpcConfig, COMPONENT};

mod api;

/// Represents an initialized rpc component where the RPC connection is open, but not yet actively
/// responding to requests.
///
/// Separating the connection binding from the server spawning allows the caller to connect other
/// components to the store without resorting to sleeps or other mechanisms to spawn dependent
/// components.
pub struct Rpc {
    api_service: api_server::ApiServer<RpcApi>,
    listener: TcpListener,
}

impl Rpc {
    pub async fn init(config: RpcConfig) -> Result<Self, ApiError> {
        info!(target: COMPONENT, %config, "Initializing server");

        let api = api::RpcApi::from_config(&config)
            .await
            .map_err(|err| ApiError::ApiInitialisationFailed(err.to_string()))?;
        let api_service = api_server::ApiServer::new(api);

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

    /// Serves the RPC API.
    ///
    /// Note: this blocks until the server dies.
    pub async fn serve(self) -> Result<(), ApiError> {
        tonic::transport::Server::builder()
            .accept_http1(true)
            .add_service(tonic_web::enable(self.api_service))
            .serve_with_incoming(TcpListenerStream::new(self.listener))
            .await
            .map_err(ApiError::ApiServeFailed)
    }
}
