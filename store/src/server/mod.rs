use std::{future::Future, net::ToSocketAddrs, sync::Arc};

use anyhow::{anyhow, Result};
use miden_node_proto::store::api_server::ApiServer as StoreApiServer;
use miden_node_utils::control_plane::{
    create_server as control_plane_create_server, ControlPlane, ControlPlaneConfig,
};
use tokio::sync::oneshot::Receiver;
use tonic::transport::Server;
use tracing::{error, info, instrument};

use crate::{config::StoreConfig, db::Db, state::State, COMPONENT};

mod api;

pub async fn serve(
    store_config: StoreConfig,
    control_plane_config: ControlPlaneConfig,
    db: Db,
) -> Result<()> {
    let mut control_plane = ControlPlane::new();

    let store_shutdown = control_plane.shutdown_waiter()?;
    let store_server = create_server(store_config, db, store_shutdown).await?;
    let control_plane_server =
        control_plane_create_server(control_plane_config, control_plane).await?;

    let (control_plane_res, store_res) =
        tokio::join!(tokio::spawn(control_plane_server), tokio::spawn(store_server));

    match control_plane_res {
        Ok(Ok(_)) => info!(COMPONENT, "Control plane successfully shut down"),
        Ok(Err(err)) => error!(COMPONENT, "Control plane shut down with an error {err:?}"),
        Err(_) => error!(COMPONENT, "Control plane join failed"),
    }
    match store_res {
        Ok(Ok(_)) => info!(COMPONENT, "Store successfully shut down"),
        Ok(Err(err)) => error!(COMPONENT, "Store shut down with an error {err:?}"),
        Err(_) => error!(COMPONENT, "Store join failed"),
    }

    Ok(())
}

/// Configures the store service and returns a future to execute it.
#[instrument(skip_all)]
pub async fn create_server(
    config: StoreConfig,
    db: Db,
    shutdown: Receiver<()>,
) -> Result<impl Future<Output = Result<()>>> {
    let endpoint = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = endpoint.to_socket_addrs()?.collect();

    let state = Arc::new(State::load(db).await?);
    let svc = StoreApiServer::new(api::StoreApi { state });

    info!(
        public_host = config.endpoint.host,
        public_port = config.endpoint.port,
        COMPONENT,
        "Store server initialized",
    );

    Ok(async move {
        Server::builder()
            .add_service(svc)
            .serve_with_shutdown(addrs[0], async {
                match shutdown.await {
                    Ok(_) => info!(COMPONENT, "Store shutdown"),
                    Err(_) => error!(COMPONENT, "Store channel closed"),
                }
            })
            .await
            .map_err(|e| anyhow!("Server failed: {e:?}"))
    })
}
