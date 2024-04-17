use std::{net::ToSocketAddrs, sync::Arc};

use miden_node_proto::generated::store::api_server;
use miden_node_utils::errors::ApiError;
use tonic::transport::Server;
use tracing::info;

use crate::{config::StoreConfig, db::Db, state::State, COMPONENT};

mod api;

// STORE INITIALIZER
// ================================================================================================

pub async fn serve(config: StoreConfig, db: Db) -> Result<(), ApiError> {
    info!(target: COMPONENT, %config, "Initializing server");

    let state = Arc::new(
        State::load(db)
            .await
            .map_err(|err| ApiError::DatabaseConnectionFailed(err.to_string()))?,
    );
    let store = api_server::ApiServer::new(api::StoreApi { state });

    info!(target: COMPONENT, "Server initialized");

    let addr = config
        .endpoint
        .to_socket_addrs()
        .map_err(|err| ApiError::EndpointToSocketFailed(err.to_string()))?
        .next()
        .ok_or("Failed to resolve address.")
        .map_err(|err| ApiError::AddressResolutionFailed(err.to_string()))?;

    Server::builder()
        .add_service(store)
        .serve(addr)
        .await
        .map_err(|err| ApiError::ApiServeFailed(err.to_string()))?;

    Ok(())
}
