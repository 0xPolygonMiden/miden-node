use crate::config::BlockProducerConfig;
use anyhow::Result;
use miden_node_proto::{
    block_producer::api_server, requests::SubmitProvenTransactionRequest,
    responses::SubmitProvenTransactionResponse,
};
use std::net::ToSocketAddrs;
use tonic::{transport::Server, Status};
use tracing::info;

pub async fn serve(config: BlockProducerConfig) -> Result<()> {
    let host_port = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = host_port.to_socket_addrs()?.collect();

    let db = api_server::ApiServer::new(BlockProducerApi {});

    info!(host = config.endpoint.host, port = config.endpoint.port, "Server initialized",);
    Server::builder().add_service(db).serve(addrs[0]).await?;

    Ok(())
}

pub struct BlockProducerApi {}

#[tonic::async_trait]
impl api_server::Api for BlockProducerApi {
    async fn submit_proven_transaction(
        &self,
        _request: tonic::Request<SubmitProvenTransactionRequest>,
    ) -> Result<tonic::Response<SubmitProvenTransactionResponse>, Status> {
        todo!()
    }
}
