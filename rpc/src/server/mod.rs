use std::{future::Future, net::ToSocketAddrs};

use anyhow::{anyhow, Result};
use miden_node_proto::rpc::api_server;
use miden_node_utils::control_plane::{
    create_server as control_plane_create_server, ControlPlane, ControlPlaneConfig,
};
use tokio::sync::oneshot::Receiver;
use tonic::transport::Server;
use tracing::{error, info, instrument};

use crate::{config::RpcConfig, COMPONENT};

mod api;

pub async fn serve(
    config: RpcConfig,
    control_plane_config: ControlPlaneConfig,
) -> Result<()> {
    let mut control_plane = ControlPlane::new();

    let shutdown = control_plane.shutdown_waiter()?;
    let rpc_server = create_server(config, shutdown).await?;

    let control_plane_server =
        control_plane_create_server(control_plane_config, control_plane).await?;

    let (control_plane_res, store_res) =
        tokio::join!(tokio::spawn(control_plane_server), tokio::spawn(rpc_server));

    match control_plane_res {
        Ok(Ok(_)) => info!(COMPONENT, "Control plane successfully shut down"),
        Ok(Err(err)) => error!(COMPONENT, "Control plane shut down with an error {err:?}"),
        Err(_) => error!(COMPONENT, "Control plane join failed"),
    }
    match store_res {
        Ok(Ok(_)) => info!(COMPONENT, "RPC successfully shut down"),
        Ok(Err(err)) => error!(COMPONENT, "RPC shut down with an error {err:?}"),
        Err(_) => error!(COMPONENT, "RPC join failed"),
    }

    Ok(())
}

/// Configures the RPC service and returns a future to execute it.
#[instrument(skip_all)]
pub async fn create_server(
    config: RpcConfig,
    shutdown: Receiver<()>,
) -> Result<impl Future<Output = Result<()>>> {
    let endpoint = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = endpoint.to_socket_addrs()?.collect();

    let api = api::RpcApi::from_config(&config).await?;
    let svc = api_server::ApiServer::new(api);

    info!(
        host = config.endpoint.host,
        port = config.endpoint.port,
        COMPONENT,
        "RPC server initialized"
    );

    Ok(async move {
        Server::builder()
            .add_service(svc)
            .serve_with_shutdown(addrs[0], async {
                match shutdown.await {
                    Ok(_) => info!(COMPONENT, "Rpc shutdown"),
                    Err(_) => error!(COMPONENT, "Rpc channel closed"),
                }
            })
            .await
            .map_err(|e| anyhow!("Server failed: {e:?}"))
    })
}
