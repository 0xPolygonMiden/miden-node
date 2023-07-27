use crate::config::RpcConfig;
use anyhow::Result;
use miden_node_proto::{digest, rpc, store::api_client};
use std::{net::ToSocketAddrs, sync::Arc};
use tonic::{
    transport::{Channel, Error, Server},
    Request, Response, Status, Streaming,
};
use tracing::info;

pub struct RpcApi {
    store: Arc<api_client::ApiClient<Channel>>,
}

impl RpcApi {
    async fn from_config(config: &RpcConfig) -> Result<Self, Error> {
        let client = api_client::ApiClient::connect(config.store.clone()).await?;
        info!(store = config.store, "Store client initialized",);
        Ok(Self {
            store: Arc::new(client),
        })
    }
}

#[tonic::async_trait]
impl rpc::api_server::Api for RpcApi {
    type CheckNullifiersStream = Streaming<rpc::ResponseBytes>;

    async fn check_nullifiers(
        &self,
        _request: Request<Streaming<digest::Digest>>,
    ) -> Result<Response<Self::CheckNullifiersStream>, Status> {
        todo!()
    }
}

pub async fn serve(config: RpcConfig) -> Result<()> {
    let host_port = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = host_port.to_socket_addrs()?.collect();

    let api = RpcApi::from_config(&config).await?;
    let rpc = rpc::api_server::ApiServer::new(api);

    info!(host = config.endpoint.host, port = config.endpoint.port, "Server initialized",);

    Server::builder().add_service(rpc).serve(addrs[0]).await?;

    Ok(())
}
