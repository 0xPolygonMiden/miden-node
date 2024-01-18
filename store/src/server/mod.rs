use std::{net::ToSocketAddrs, sync::Arc};

use anyhow::Result;
use miden_node_proto::store::api_server;
use tonic::transport::Server;
use tracing::{info, instrument};

use crate::{config::StoreConfig, db::Db, state::State};

mod api;

// STORE INITIALIZER
// ================================================================================================

#[instrument(skip(config, db))]
pub async fn serve(
    config: StoreConfig,
    db: Db,
) -> Result<()> {
    let endpoint = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = endpoint.to_socket_addrs()?.collect();

    let state = Arc::new(State::load(db).await?);
    let store = api_server::ApiServer::new(api::StoreApi { state });

    info!(host = config.endpoint.host, port = config.endpoint.port, "Server initialized",);
    Server::builder().add_service(store).serve(addrs[0]).await?;

    Ok(())
}
