use std::{path::PathBuf, sync::Arc};

use miden_node_proto::generated::store::api_server;
use miden_node_utils::{errors::ApiError, tracing::grpc::store_trace_fn};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::{COMPONENT, GENESIS_STATE_FILENAME, blocks::BlockStore, db::Db, state::State};

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
    /// Performs initialization tasks required before [`serve`](Self::serve) can be called.
    pub async fn init(listener: TcpListener, data_directory: PathBuf) -> Result<Self, ApiError> {
        info!(target: COMPONENT, endpoint=?listener, ?data_directory, "Loading database");

        let block_store = data_directory.join("blocks");
        let block_store = Arc::new(BlockStore::new(block_store).await?);

        let database_filepath = data_directory.join("miden-store.sqlite3");
        let genesis_filepath = data_directory.join(GENESIS_STATE_FILENAME);

        let db = Db::setup(
            database_filepath,
            &genesis_filepath.to_string_lossy(),
            Arc::clone(&block_store),
        )
        .await
        .map_err(|err| ApiError::ApiInitialisationFailed(err.to_string()))?;

        let state = Arc::new(
            State::load(db, block_store)
                .await
                .map_err(|err| ApiError::DatabaseConnectionFailed(err.to_string()))?,
        );

        let api_service = api_server::ApiServer::new(api::StoreApi { state });

        info!(target: COMPONENT, "Database loaded");

        Ok(Self { api_service, listener })
    }

    /// Serves the store's RPC API.
    ///
    /// Note: this blocks until the server dies.
    pub async fn serve(self) -> Result<(), ApiError> {
        // Build the gRPC server with the API service and trace layer.
        tonic::transport::Server::builder()
            .layer(TraceLayer::new_for_grpc().make_span_with(store_trace_fn))
            .add_service(self.api_service)
            .serve_with_incoming(TcpListenerStream::new(self.listener))
            .await
            .map_err(ApiError::ApiServeFailed)
    }
}
