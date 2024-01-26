use std::{net::ToSocketAddrs, sync::Arc};

use anyhow::Result;
use miden_node_proto::store::api_server;
use tonic::transport::Server;
use tracing::{info, instrument};

use crate::{config::StoreConfig, db::Db, state::State, COMPONENT};

mod api;

// STORE INITIALIZER
// ================================================================================================

#[instrument(target = "miden-store", skip_all)]
pub async fn serve(
    config: StoreConfig,
    db: Db,
) -> Result<()> {
    info!(target: COMPONENT, %config, "Initializing server");

    let endpoint = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = endpoint.to_socket_addrs()?.collect();

    let state = Arc::new(State::load(db).await?);
    let store = api_server::ApiServer::new(api::StoreApi { state });

    info!(target: COMPONENT, "Server initialized");

    Server::builder().add_service(store).serve(addrs[0]).await?;

    Ok(())
}
