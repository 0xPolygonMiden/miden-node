use std::{sync::Arc, time::Duration};

use miden_node_proto::generated::store::api_server;
use miden_node_utils::errors::ApiError;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tower_http::{classify::GrpcFailureClass, trace::TraceLayer};
use tracing::{error, info, Span};

use crate::{blocks::BlockStore, config::StoreConfig, db::Db, state::State, COMPONENT};

mod api;

/// Represents an initialized store component where the RPC connection is open, but not yet actively
/// responding to requests.
///
/// Separating the connection binding from the server spawning allows the caller to connect other
/// components to the store without resorting to sleeps or other mechanisms to spawn dependent
/// components.
pub struct Store {
    api_service: api_server::ApiServer<api::StoreApi>,
    listener: TcpListener,
}

impl Store {
    /// Loads the required database data and initializes the TCP listener without
    /// serving the API yet. Incoming requests will be queued until [`serve`](Self::serve) is
    /// called.
    pub async fn init(config: StoreConfig) -> Result<Self, ApiError> {
        info!(target: COMPONENT, %config, "Loading database");

        let block_store = Arc::new(BlockStore::new(config.blockstore_dir.clone()).await?);

        let db = Db::setup(config.clone(), Arc::clone(&block_store))
            .await
            .map_err(|err| ApiError::ApiInitialisationFailed(err.to_string()))?;

        let state = Arc::new(
            State::load(db, block_store)
                .await
                .map_err(|err| ApiError::DatabaseConnectionFailed(err.to_string()))?,
        );

        let api_service = api_server::ApiServer::new(api::StoreApi { state });

        let addr = config
            .endpoint
            .socket_addrs(|| None)
            .map_err(ApiError::EndpointToSocketFailed)?
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::AddressResolutionFailed(config.endpoint.to_string()))?;

        let listener = TcpListener::bind(addr).await?;

        info!(target: COMPONENT, "Database loaded");

        Ok(Self { api_service, listener })
    }

    /// Serves the store's RPC API.
    ///
    /// Note: this blocks until the server dies.
    pub async fn serve(self) -> Result<(), ApiError> {
        // Configure the trace layer with callbacks.
        let trace_layer = TraceLayer::new_for_grpc()
            .make_span_with(miden_node_utils::tracing::grpc::store_trace_fn)
            .on_request(|request: &http::Request<_>, _span: &Span| {
                info!(
                    "request: {} {} {} {:?}",
                    request.method(),
                    request.uri().host().unwrap_or("unknown_host"),
                    request.uri().path(),
                    request.headers()
                );
            })
            .on_response(|response: &http::Response<_>, latency: Duration, _span: &Span| {
                info!("response: {} {:?}", response.status(), latency);
            })
            .on_failure(|error: GrpcFailureClass, latency: Duration, _span: &Span| {
                error!("error: {} {:?}", error, latency);
            });
        // Build the gRPC server with the API service and trace layer.
        tonic::transport::Server::builder()
            .layer(trace_layer)
            .add_service(self.api_service)
            .serve_with_incoming(TcpListenerStream::new(self.listener))
            .await
            .map_err(ApiError::ApiServeFailed)
    }
}
