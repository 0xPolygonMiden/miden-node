use std::net::ToSocketAddrs;

use miden_node_proto::generated::rpc::api_server;
use tonic::transport::{Error, Server};
use tracing::info;

use crate::{config::RpcConfig, COMPONENT};

mod api;

// RPC INITIALIZER
// ================================================================================================

pub async fn serve(config: RpcConfig) -> Result<(), Error> {
    info!(target: COMPONENT, %config, "Initializing server");

    let api = api::RpcApi::from_config(&config).await?;
    let rpc = api_server::ApiServer::new(api);

    info!(target: COMPONENT, "Server initialized");

    let addr = config
        .endpoint
        .to_socket_addrs()
        .expect("Failed to turn address into socket address.")
        .next()
        .expect("Failed to resolve address.");

    Server::builder().add_service(rpc).serve(addr).await?;

    Ok(())
}
