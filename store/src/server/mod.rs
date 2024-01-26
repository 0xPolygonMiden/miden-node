use std::{net::ToSocketAddrs, sync::Arc};

use anyhow::{anyhow, Result};
use miden_node_proto::store::api_server;
use tonic::transport::Server;
use tracing::{info, instrument};

use crate::{config::StoreConfig, db::Db, state::State, COMPONENT};

mod api;

// STORE INITIALIZER
// ================================================================================================

#[instrument(skip(config, db))]
pub async fn serve(
    config: StoreConfig,
    db: Db,
) -> Result<()> {
    let state = Arc::new(State::load(db).await?);
    let store = api_server::ApiServer::new(api::StoreApi { state });

    info!(
        host = config.endpoint.host,
        port = config.endpoint.port,
        COMPONENT,
        "Server initialized",
    );

    let addr = config
        .endpoint
        .to_socket_addrs()?
        .next()
        .ok_or(anyhow!("Couldn't resolve server address"))?;
    Server::builder().add_service(store).serve(addr).await?;

    Ok(())
}
