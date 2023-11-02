use crate::config::StoreConfig;
use crate::db::Db;
use anyhow::Result;
use miden_crypto::{
    hash::rpo::RpoDigest,
    merkle::{Mmr, NodeIndex, SimpleSmt, TieredSmt},
    Felt, FieldElement, StarkField, Word,
};
use miden_node_proto::responses::AccountInputRecord;
use miden_node_proto::{
    digest::Digest,
    error::ParseError,
    merkle::MerklePath,
    mmr::MmrDelta,
    requests::{
        CheckNullifiersRequest, GetBlockHeaderByNumberRequest, GetBlockInputsRequest,
        SyncStateRequest,
    },
    responses::{
        CheckNullifiersResponse, GetBlockHeaderByNumberResponse, GetBlockInputsResponse,
        SyncStateResponse,
    },
    store::api_server,
    tsmt::{self, NullifierLeaf},
};
use miden_objects::BlockHeader;
use std::{net::ToSocketAddrs, sync::Arc};
use tokio::{sync::RwLock, time::Instant};
use tonic::{transport::Server, Response, Status};
use tracing::{info, instrument};

const ACCOUNT_DB_DEPTH: u8 = 64;

pub struct StoreApi {
    db: Db,
    nullifier_tree: Arc<RwLock<TieredSmt>>,
    chain_mmr: Arc<RwLock<Mmr>>,
    account_tree: Arc<RwLock<SimpleSmt>>,
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

    async fn get_block_header_by_number(
        &self,
        request: tonic::Request<GetBlockHeaderByNumberRequest>,
    ) -> Result<Response<GetBlockHeaderByNumberResponse>, Status> {
        let request = request.into_inner();
        let block_header =
            self.db.get_block_header(request.block_num).await.map_err(internal_error)?;

        Ok(Response::new(GetBlockHeaderByNumberResponse { block_header }))
    }

    async fn sync_state(
        &self,
        request: tonic::Request<SyncStateRequest>,
    ) -> Result<Response<SyncStateResponse>, Status> {
        let request = request.into_inner();

        let account_ids: Vec<u64> = request.account_ids.iter().map(|e| e.id).collect();

        let state_sync = self
            .db
            .get_state_sync(
                request.block_num,
                &account_ids,
                &request.note_tags,
                &request.nullifiers,
            )
            .await
            .map_err(internal_error)?;

        // scope to read from the mmr
        let (delta, path): (MmrDelta, MerklePath) = {
            let mmr = self.chain_mmr.read().await;
            let delta = mmr
                .get_delta(request.block_num as usize, state_sync.block_header.block_num as usize)
                .map_err(internal_error)?
                .try_into()
                .map_err(internal_error)?;

            let proof = mmr
                .open(
                    state_sync.block_header.block_num as usize,
                    state_sync.block_header.block_num as usize,
                )
                .map_err(internal_error)?;

            (delta, proof.merkle_path.into())
        };

        let notes = state_sync.notes.into_iter().map(|v| v.into()).collect();
        Ok(Response::new(SyncStateResponse {
            chain_tip: state_sync.chain_tip,
            block_header: Some(state_sync.block_header),
            mmr_delta: Some(delta),
            block_path: Some(path),
            accounts: state_sync.account_updates,
            notes,
            nullifiers: state_sync.nullifiers,
        }))
    }

    async fn get_block_inputs(
        &self,
        request: tonic::Request<GetBlockInputsRequest>,
    ) -> Result<Response<GetBlockInputsResponse>, Status> {
        let request = request.into_inner();

        let latest = self
            .db
            .get_block_header(None)
            .await
            .map_err(internal_error)?
            .ok_or(Status::internal("Latest block not found"))?;

        // scope to read from the mmr
        let accumulator = {
            let mmr = self.chain_mmr.read().await;
            mmr.peaks(latest.block_num as usize).map_err(internal_error)?
        };
        let mmr_peaks = accumulator.peaks().iter().map(|d| d.into()).collect();

        // scope to read from the accounts
        let mut account_states = Vec::with_capacity(request.account_ids.len());
        {
            let accounts = self.account_tree.read().await;
            for account_id in request.account_ids {
                let account_hash = accounts.get_leaf(account_id.id).map_err(internal_error)?;
                let proof = accounts.get_leaf_path(account_id.id).map_err(internal_error)?;
                account_states.push(AccountInputRecord {
                    account_id: Some(account_id),
                    account_hash: Some(account_hash.into()),
                    proof: Some(proof.into()),
                });
            }
        }

        Ok(Response::new(GetBlockInputsResponse {
            block_header: Some(latest),
            mmr_peaks,
            account_states,
            // TODO: nullifiers blocked by changes in crypto repo
            nullifiers: vec![],
        }))
    }
}

pub async fn serve(
    config: StoreConfig,
    mut db: Db,
) -> Result<()> {
    let host_port = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = host_port.to_socket_addrs()?.collect();

    let nullifier_data = load_nullifier_tree(&mut db).await?;
    let nullifier_lock = Arc::new(RwLock::new(nullifier_data));
    let mmr_data = load_mmr(&mut db).await?;
    let mmr_lock = Arc::new(RwLock::new(mmr_data));
    let accounts_data = load_accounts(&mut db).await?;
    let accounts_lock = Arc::new(RwLock::new(accounts_data));
    let db = api_server::ApiServer::new(StoreApi {
        db,
        nullifier_tree: nullifier_lock,
        chain_mmr: mmr_lock,
        account_tree: accounts_lock,
    });

    info!(host = config.endpoint.host, port = config.endpoint.port, "Server initialized",);
    Server::builder().add_service(db).serve(addrs[0]).await?;

    Ok(())
}

// UTILITIES
// ================================================================================================

#[instrument(skip(db))]
async fn load_nullifier_tree(db: &mut Db) -> Result<TieredSmt> {
    let nullifiers = db.get_nullifiers().await?;
    let len = nullifiers.len();
    let leaves = nullifiers.into_iter().map(|(nullifier, block)| {
        (nullifier, [Felt::new(block as u64), Felt::ZERO, Felt::ZERO, Felt::ZERO])
    });

    let now = Instant::now();
    let nullifier_tree = TieredSmt::with_entries(leaves)?;
    let elapsed = now.elapsed().as_secs();

    info!(num_of_leaves = len, tree_construction = elapsed, "Loaded nullifier tree");
    Ok(nullifier_tree)
}

#[instrument(skip(db))]
async fn load_mmr(db: &mut Db) -> Result<Mmr> {
    let block_hashes: Result<Vec<RpoDigest>, ParseError> = db
        .get_block_headers()
        .await?
        .into_iter()
        .map(|b| b.try_into().map(|b: BlockHeader| b.hash()))
        .collect();

    let mmr: Mmr = block_hashes?.into();
    Ok(mmr)
}

#[instrument(skip(db))]
async fn load_accounts(db: &mut Db) -> Result<SimpleSmt> {
    let account_data: Result<Vec<(u64, Word)>> = db
        .get_account_hashes()
        .await?
        .into_iter()
        .map(|(id, account_hash)| Ok((id, account_hash.try_into()?)))
        .collect();

    let smt = SimpleSmt::with_leaves(ACCOUNT_DB_DEPTH, account_data?)?;

    Ok(smt)
}

/// Formats an error
fn internal_error<E: core::fmt::Debug>(err: E) -> Status {
    Status::internal(format!("{:?}", err))
}
