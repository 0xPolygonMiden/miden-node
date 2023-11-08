use crate::{config::StoreConfig, db::Db, state::State};
use anyhow::Result;
use miden_crypto::hash::rpo::RpoDigest;
use miden_node_proto::{
    conversion::convert,
    digest::Digest,
    error::ParseError,
    requests::{
        CheckNullifiersRequest, GetBlockHeaderByNumberRequest, GetBlockInputsRequest,
        GetTransactionInputsRequest, SyncStateRequest,
    },
    responses::{
        CheckNullifiersResponse, GetBlockHeaderByNumberResponse, GetBlockInputsResponse,
        GetTransactionInputsResponse, SyncStateResponse,
    },
    store::api_server,
};
use std::{net::ToSocketAddrs, sync::Arc};
use tonic::{transport::Server, Response, Status};
use tracing::info;

// STORE INITIALIZER
// ================================================================================================

pub async fn serve(
    config: StoreConfig,
    db: Db,
) -> Result<()> {
    let host_port = (config.endpoint.host.as_ref(), config.endpoint.port);
    let addrs: Vec<_> = host_port.to_socket_addrs()?.collect();

    let state = Arc::new(State::load(db).await?);
    let db = api_server::ApiServer::new(StoreApi { state });

    info!(host = config.endpoint.host, port = config.endpoint.port, "Server initialized",);
    Server::builder().add_service(db).serve(addrs[0]).await?;

    Ok(())
}

// STORE API
// ================================================================================================

pub struct StoreApi {
    state: Arc<State>,
}

#[tonic::async_trait]
impl api_server::Api for StoreApi {
    // CLIENT ENDPOINTS
    // --------------------------------------------------------------------------------------------

    /// Returns block header for the specified block number.
    ///
    /// If the block number is not provided, block header for the latest block is returned.
    async fn get_block_header_by_number(
        &self,
        request: tonic::Request<GetBlockHeaderByNumberRequest>,
    ) -> Result<Response<GetBlockHeaderByNumberResponse>, Status> {
        let request = request.into_inner();
        let block_header =
            self.state.get_block_header(request.block_num).await.map_err(internal_error)?;

        Ok(Response::new(GetBlockHeaderByNumberResponse { block_header }))
    }

    /// Returns info on whether the specified nullifiers have been consumed.
    ///
    /// This endpoint also returns Merkle authentication path for each requested nullifier which can
    /// be verified against the latest root of the nullifier database.
    async fn check_nullifiers(
        &self,
        request: tonic::Request<CheckNullifiersRequest>,
    ) -> Result<Response<CheckNullifiersResponse>, Status> {
        // Validate the nullifiers and convert them to RpoDigest values. Stop on first error.
        let request = request.into_inner();
        let nullifiers = validate_nullifiers(&request.nullifiers)?;

        // Query the state for the request's nullifiers
        let proofs = self.state.check_nullifiers(&nullifiers).await;

        Ok(Response::new(CheckNullifiersResponse {
            proofs: convert(proofs),
        }))
    }

    /// Returns info which can be used by the client to sync up to the latest state of the chain
    /// for the objects the client is interested in.
    async fn sync_state(
        &self,
        request: tonic::Request<SyncStateRequest>,
    ) -> Result<Response<SyncStateResponse>, Status> {
        let request = request.into_inner();

        let account_ids: Vec<u64> = request.account_ids.iter().map(|e| e.id).collect();

        let (state, delta, path) = self
            .state
            .sync_state(request.block_num, &account_ids, &request.note_tags, &request.nullifiers)
            .await
            .map_err(internal_error)?;

        Ok(Response::new(SyncStateResponse {
            chain_tip: state.chain_tip,
            block_header: Some(state.block_header),
            mmr_delta: Some(delta.into()),
            block_path: Some(path.into()),
            accounts: state.account_updates,
            notes: convert(state.notes),
            nullifiers: state.nullifiers,
        }))
    }

    // BLOCK PRODUCER ENDPOINTS
    // --------------------------------------------------------------------------------------------

    /// Returns data needed by the block producer to construct and prove the next block.
    async fn get_block_inputs(
        &self,
        request: tonic::Request<GetBlockInputsRequest>,
    ) -> Result<Response<GetBlockInputsResponse>, Status> {
        let request = request.into_inner();

        let nullifiers = validate_nullifiers(&request.nullifiers)?;
        let account_ids: Vec<u64> = request.account_ids.iter().map(|e| e.id).collect();

        let (latest, accumulator, account_states) = self
            .state
            .get_block_inputs(&account_ids, &nullifiers)
            .await
            .map_err(internal_error)?;

        Ok(Response::new(GetBlockInputsResponse {
            block_header: Some(latest),
            mmr_peaks: convert(accumulator.peaks()),
            account_states: convert(account_states),
            // TODO: nullifiers blocked by changes in crypto repo
            nullifiers: vec![],
        }))
    }

    async fn get_transaction_inputs(
        &self,
        request: tonic::Request<GetTransactionInputsRequest>,
    ) -> Result<Response<GetTransactionInputsResponse>, Status> {
        let request = request.into_inner();

        let nullifiers = validate_nullifiers(&request.nullifiers)?;
        let account_ids: Vec<u64> = request.account_ids.iter().map(|e| e.id).collect();

        let (accounts, nullifiers_blocks) = self
            .state
            .get_transaction_inputs(&account_ids, &nullifiers)
            .await
            .map_err(internal_error)?;

        Ok(Response::new(GetTransactionInputsResponse {
            account_states: convert(accounts),
            nullifiers: convert(nullifiers_blocks),
        }))
    }
}

// UTILITIES
// ================================================================================================

/// Formats an error
fn internal_error<E: core::fmt::Debug>(err: E) -> Status {
    Status::internal(format!("{:?}", err))
}

fn invalid_argument<E: core::fmt::Debug>(err: E) -> Status {
    Status::invalid_argument(format!("{:?}", err))
}

fn validate_nullifiers(nullifiers: &[Digest]) -> Result<Vec<RpoDigest>, Status> {
    nullifiers
        .into_iter()
        .map(|v| v.try_into())
        .collect::<Result<Vec<RpoDigest>, ParseError>>()
        .map_err(|_| invalid_argument("Digest field is not in the modulus range"))
}
