use std::{net::ToSocketAddrs, sync::Arc};

use miden_node_proto::generated::store::api_server;
use miden_node_utils::errors::ApiError;
use tonic::transport::Server;
use tracing::info;

use crate::{blocks::BlockStore, config::StoreConfig, db::Db, state::State, COMPONENT};

mod api;

// STORE INITIALIZER
// ================================================================================================

pub async fn serve(config: StoreConfig) -> Result<(), ApiError> {
    info!(target: COMPONENT, %config, "Initializing server");

    let block_store = Arc::new(BlockStore::new(config.blockstore_dir.clone()).await?);

    let db = Db::setup(config.clone(), Arc::clone(&block_store))
        .await
        .map_err(|err| ApiError::ApiInitialisationFailed(err.to_string()))?;

    let state = Arc::new(
        State::load(db, block_store)
            .await
            .map_err(|err| ApiError::DatabaseConnectionFailed(err.to_string()))?,
    );

    let store = api_server::ApiServer::new(api::StoreApi { state });

    info!(target: COMPONENT, "Server initialized");

    let addr = config
        .endpoint
        .to_socket_addrs()
        .map_err(ApiError::EndpointToSocketFailed)?
        .next()
        .ok_or_else(|| ApiError::AddressResolutionFailed(config.endpoint.to_string()))?;

    Server::builder()
        .add_service(store)
        .serve(addr)
        .await
        .map_err(ApiError::ApiServeFailed)?;

    Ok(())
}
