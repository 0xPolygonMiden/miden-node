use std::net::ToSocketAddrs;

use anyhow::{anyhow, Result};
use miden_node_proto::rpc::api_server;
use tonic::transport::Server;
use tracing::{info, instrument};

use crate::{config::RpcConfig, COMPONENT};

mod api;

// RPC INITIALIZER
// ================================================================================================
#[instrument(target = "miden-rpc", name = "rpc", skip_all)]
pub async fn serve(config: RpcConfig) -> Result<()> {
    info!(target: COMPONENT, %config, "Initializing server");

    let api = api::RpcApi::from_config(&config).await?;
    let rpc = api_server::ApiServer::new(api);

    info!(target: COMPONENT, "Server initialized");

    let addr = config
        .endpoint
        .to_socket_addrs()?
        .next()
        .ok_or(anyhow!("Couldn't resolve server address"))?;
    Server::builder().add_service(rpc).serve(addr).await?;

    Ok(())
}
