use std::net::ToSocketAddrs;
use tracing::info;
use anyhow::Result;

use crate::config::StoreConfig;
use tonic::{transport::Server, Response, Status, Streaming};

use miden_node_proto::store;

#[derive(Default)]
pub struct DBApi {}

#[tonic::async_trait]
impl store::api_server::Api for DBApi {
    type CheckNullifiersStream = Streaming<store::CheckNullifiersResponse>;

    async fn check_nullifiers(
        &self,
        _request: tonic::Request<Streaming<store::CheckNullifiersRequest>>,
    ) -> Result<Response<Self::CheckNullifiersStream>, Status>
    {
        todo!()
    }
}

pub async fn serve(config: StoreConfig) -> Result<()> {
    let host_port = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = host_port.to_socket_addrs()?.collect();

    info!(
        host = config.endpoint.host,
        port = config.endpoint.port,
        "Server initialized",
    );

    let db = store::api_server::ApiServer::new(DBApi::default());
    Server::builder().add_service(db).serve(addrs[0]).await?;

    Ok(())
}
