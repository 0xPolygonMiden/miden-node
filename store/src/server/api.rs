use crate::config::StoreConfig;
use crate::db::Db;
use anyhow::Result;
use miden_crypto::{hash::rpo::RpoDigest, merkle::TieredSmt, Felt, FieldElement, StarkField};
use miden_node_proto::{
    digest::Digest,
    store::{
        api_server, CheckNullifiersRequest, CheckNullifiersResponse,
        FetchBlockHeaderByNumberRequest, FetchBlockHeaderByNumberResponse,
    },
    tsmt::{self, NullifierLeaf},
};
use std::{net::ToSocketAddrs, sync::Arc};
use tokio::{sync::RwLock, time::Instant};
use tonic::{transport::Server, Response, Status};
use tracing::{info, instrument};

pub struct StoreApi {
    db: Db,
    nullifier_tree: Arc<RwLock<TieredSmt>>,
}

#[tonic::async_trait]
impl api_server::Api for StoreApi {
    async fn check_nullifiers(
        &self,
        request: tonic::Request<CheckNullifiersRequest>,
    ) -> Result<Response<CheckNullifiersResponse>, Status> {
        // Validate the nullifiers and convert them to RpoDigest values. Stop on first error.
        let nullifiers = request
            .into_inner()
            .nullifiers
            .into_iter()
            .map(|v| {
                v.try_into()
                    .or(Err(Status::invalid_argument("Digest field is not in the modulos range")))
            })
            .collect::<Result<Vec<RpoDigest>, Status>>()?;

        let nullifier_tree = self.nullifier_tree.read().await;

        let proofs = nullifiers
            .into_iter()
            .map(|nullifier| {
                let proof: miden_crypto::merkle::TieredSmtProof = nullifier_tree.prove(nullifier);

                let (path, entries) = proof.into_parts();

                let merkle_path: Vec<Digest> = path.into_iter().map(|e| e.into()).collect();

                let leaves: Vec<NullifierLeaf> = entries
                    .into_iter()
                    .map(|(key, value)| NullifierLeaf {
                        key: Some(key.into()),
                        value: value[3].as_int(),
                    })
                    .collect();

                tsmt::NullifierProof {
                    merkle_path,
                    leaves,
                }
            })
            .collect();

        Ok(Response::new(CheckNullifiersResponse { proofs }))
    }

    async fn fetch_block_header_by_number(
        &self,
        request: tonic::Request<FetchBlockHeaderByNumberRequest>,
    ) -> Result<Response<FetchBlockHeaderByNumberResponse>, Status> {
        let request = request.into_inner();
        let block_header = self
            .db
            .get_block_header(request.block_num)
            .await
            .map_err(|err| Status::internal(format!("{:?}", err)))?;

        Ok(Response::new(FetchBlockHeaderByNumberResponse { block_header }))
    }
}

#[instrument(skip(db))]
async fn load_nullifier_tree(db: &mut Db) -> Result<TieredSmt> {
    let nullifiers = db.get_nullifiers().await?;
    let len = nullifiers.len();
    let leaves = nullifiers.into_iter().map(|(nullifier, block)| {
        (nullifier, [Felt::new(block), Felt::ZERO, Felt::ZERO, Felt::ZERO])
    });

    let now = Instant::now();
    let nullifier_tree = TieredSmt::with_entries(leaves)?;
    let elapsed = now.elapsed().as_secs();

    info!(num_of_leaves = len, tree_construction = elapsed, "Loaded nullifier tree");
    Ok(nullifier_tree)
}

pub async fn serve(
    config: StoreConfig,
    mut db: Db,
) -> Result<()> {
    let host_port = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = host_port.to_socket_addrs()?.collect();

    let tree_data = load_nullifier_tree(&mut db).await?;
    let tree_lock = Arc::new(RwLock::new(tree_data));
    let db = api_server::ApiServer::new(StoreApi {
        db,
        nullifier_tree: tree_lock,
    });

    info!(host = config.endpoint.host, port = config.endpoint.port, "Server initialized",);
    Server::builder().add_service(db).serve(addrs[0]).await?;

    Ok(())
}
