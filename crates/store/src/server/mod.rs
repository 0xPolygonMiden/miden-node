use std::{sync::Arc, time::Duration};

use miden_node_proto::generated::store::api_server;
use miden_node_utils::errors::ApiError;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tracing::info;

use crate::{
    blocks::BlockStore, config::StoreConfig, db::Db, server::db_maintenance::DbMaintenance,
    state::State, COMPONENT,
};

mod api;
mod db_maintenance;

/// Represents an initialized store component where the RPC connection is open, but not yet actively
/// responding to requests.
///
/// Separating the connection binding from the server spawning allows the caller to connect other
/// components to the store without resorting to sleeps or other mechanisms to spawn dependent
/// components.
pub struct Store {
    api_service: api_server::ApiServer<api::StoreApi>,
    db_maintenance_service: DbMaintenance,
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

        let db_maintenance_service = DbMaintenance::new(
            Arc::clone(&state),
            Duration::from_secs(config.db_optimization_interval_secs),
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

        Ok(Self {
            api_service,
            db_maintenance_service,
            listener,
        })
    }

    /// Serves the store's RPC API.
    ///
    /// Note: this blocks until the server dies.
    pub async fn serve(self) -> Result<(), ApiError> {
        let db_maintenance_service = self.db_maintenance_service.run();
        let api_service = tonic::transport::Server::builder()
            .trace_fn(miden_node_utils::tracing::grpc::store_trace_fn)
            .add_service(self.api_service)
            .serve_with_incoming(TcpListenerStream::new(self.listener));

        tokio::select! {
            api_service = api_service => {
                api_service.map_err(ApiError::ApiServeFailed)
            },

            _ = db_maintenance_service => {
                info!(target: COMPONENT, "Database maintenance service shutdown");

                Ok(())
            },
        }
    }
}
