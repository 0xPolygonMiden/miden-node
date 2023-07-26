use crate::config::StoreConfig;
use crate::db::Db;
use anyhow::Result;
use miden_crypto::{merkle::TieredSmt, Felt, FieldElement};
use miden_node_proto::store;
use std::net::ToSocketAddrs;
use tokio::time::Instant;
use tonic::{transport::Server, Response, Status, Streaming};
use tracing::{info, instrument};

pub struct StoreApi {
    db: Db,
    nullifier_tree: TieredSmt,
}

#[tonic::async_trait]
impl store::api_server::Api for StoreApi {
    type CheckNullifiersStream = Streaming<store::CheckNullifiersResponse>;

    async fn check_nullifiers(
        &self,
        _request: tonic::Request<Streaming<store::CheckNullifiersRequest>>,
    ) -> Result<Response<Self::CheckNullifiersStream>, Status> {
        todo!()
    }
}

#[instrument(skip(db))]
async fn load_nullifier_tree(db: &mut Db) -> Result<TieredSmt> {
    let nullifiers = db.get_nullifiers().await?;
    let len = nullifiers.len();
    let leaves = nullifiers.into_iter().map(|(nullifier, block)| {
        (
            nullifier,
            [Felt::new(block), Felt::ZERO, Felt::ZERO, Felt::ZERO],
        )
    });

    let now = Instant::now();
    let nullifier_tree = TieredSmt::with_leaves(leaves)?;
    let elapsed = now.elapsed().as_secs();

    info!(
        num_of_leaves = len,
        tree_construction = elapsed,
        "Loaded nullifier tree"
    );
    Ok(nullifier_tree)
}

pub async fn serve(config: StoreConfig, mut db: Db) -> Result<()> {
    let host_port = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = host_port.to_socket_addrs()?.collect();

    let nullifier_tree = load_nullifier_tree(&mut db).await?;
    let db = store::api_server::ApiServer::new(StoreApi { db, nullifier_tree });

    info!(
        host = config.endpoint.host,
        port = config.endpoint.port,
        "Server initialized",
    );
    Server::builder().add_service(db).serve(addrs[0]).await?;

    Ok(())
}
