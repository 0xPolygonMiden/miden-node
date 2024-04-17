use std::{net::ToSocketAddrs, sync::Arc};

use miden_node_proto::generated::store::api_server;
use tonic::transport::{Error, Server};
use tracing::info;

use crate::{config::StoreConfig, db::Db, state::State, COMPONENT};

mod api;

// STORE INITIALIZER
// ================================================================================================

pub async fn serve(config: StoreConfig, db: Db) -> Result<(), Error> {
    info!(target: COMPONENT, %config, "Initializing server");

    let state = Arc::new(State::load(db).await.expect("Failed to load database"));
    let store = api_server::ApiServer::new(api::StoreApi { state });

    info!(target: COMPONENT, "Server initialized");

    let addr = config
        .endpoint
        .to_socket_addrs()
        .expect("Failed to turn address into socket address.")
        .next()
        .expect("Failed to resolve address.");

    Server::builder().add_service(store).serve(addr).await?;

    Ok(())
}
