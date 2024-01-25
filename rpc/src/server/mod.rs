use std::net::ToSocketAddrs;

use anyhow::{anyhow, Result};
use miden_node_proto::rpc::api_server;
use tonic::transport::Server;
use tracing::info;

use crate::{config::RpcConfig, COMPONENT};

mod api;

// RPC INITIALIZER
// ================================================================================================

pub async fn serve(config: RpcConfig) -> Result<()> {
    let api = api::RpcApi::from_config(&config).await?;
    let rpc = api_server::ApiServer::new(api);

    info!(
        host = config.endpoint.host,
        port = config.endpoint.port,
        COMPONENT,
        "Server initialized"
    );

    let addr = config
        .endpoint
        .to_socket_addrs()?
        .next()
        .ok_or(anyhow!("Couldn't resolve server address"))?;
    Server::builder().add_service(rpc).serve(addr).await?;

    Ok(())
}
