use std::net::ToSocketAddrs;

use anyhow::Result;
use miden_node_proto::rpc::api_server;
use tonic::transport::Server;
use tracing::{info, instrument};

use crate::{config::RpcConfig, target};

mod api;

// RPC INITIALIZER
// ================================================================================================
#[instrument(target = "miden-rpc")]
pub async fn serve(config: RpcConfig) -> Result<()> {
    let endpoint = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = endpoint.to_socket_addrs()?.collect();

    let api = api::RpcApi::from_config(&config).await?;
    let rpc = api_server::ApiServer::new(api);

    info!(target: target!(), host = config.endpoint.host, port = config.endpoint.port, "Server initialized");

    Server::builder().add_service(rpc).serve(addrs[0]).await?;

    Ok(())
}
