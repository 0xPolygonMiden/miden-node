use std::sync::Arc;

use anyhow::Result;
use miden_crypto::hash::rpo::RpoDigest;
use miden_node_proto::{
    conversion::convert,
    digest::Digest,
    error::ParseError,
    requests::{
        ApplyBlockRequest, CheckNullifiersRequest, GetBlockHeaderByNumberRequest,
        GetBlockInputsRequest, GetTransactionInputsRequest, ListAccountsRequest, ListNotesRequest,
        ListNullifiersRequest, SyncStateRequest,
    },
    responses::{
        ApplyBlockResponse, CheckNullifiersResponse, GetBlockHeaderByNumberResponse,
        GetBlockInputsResponse, GetTransactionInputsResponse, ListAccountsResponse,
        ListNotesResponse, ListNullifiersResponse, SyncStateResponse,
    },
    store::api_server,
    tsmt::NullifierLeaf,
};
use tonic::{Response, Status};
use tracing::{debug, instrument};

use crate::{state::State, COMPONENT};

// STORE API
// ================================================================================================

pub struct StoreApi {
    pub(super) state: Arc<State>,
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

        let (state, delta) = self
            .state
            .sync_state(request.block_num, &account_ids, &request.note_tags, &request.nullifiers)
            .await
            .map_err(internal_error)?;

        Ok(Response::new(SyncStateResponse {
            chain_tip: state.chain_tip,
            block_header: Some(state.block_header),
            mmr_delta: Some(delta.into()),
            accounts: state.account_updates,
            notes: convert(state.notes),
            nullifiers: state.nullifiers,
        }))
    }

    // BLOCK PRODUCER ENDPOINTS
    // --------------------------------------------------------------------------------------------

    /// Updates the local DB by inserting a new block header and the related data.
    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(target = "miden-store", name = "store::apply_block", skip_all, err)]
    async fn apply_block(
        &self,
        request: tonic::Request<ApplyBlockRequest>,
    ) -> Result<tonic::Response<ApplyBlockResponse>, tonic::Status> {
        let request = request.into_inner();

        debug!(target: COMPONENT, ?request);

        let nullifiers = validate_nullifiers(&request.nullifiers)?;
        let accounts = request
            .accounts
            .iter()
            .map(|account_update| {
                let account_id = account_update
                    .account_id
                    .clone()
                    .ok_or(invalid_argument("Account update missing account id"))?;
                let account_hash = account_update
                    .account_hash
                    .clone()
                    .ok_or(invalid_argument("Account update missing account hash"))?;
                Ok((account_id.id, account_hash))
            })
            .collect::<Result<Vec<_>, Status>>()?;

        let block = request.block.ok_or(invalid_argument("Apply block missing block header"))?;

        let notes = request.notes;

        let _ = self.state.apply_block(block, nullifiers, accounts, notes).await;

        Ok(Response::new(ApplyBlockResponse {}))
    }

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

    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(
        target = "miden-store",
        name = "store::get_transaction_inputs",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_transaction_inputs(
        &self,
        request: tonic::Request<GetTransactionInputsRequest>,
    ) -> Result<Response<GetTransactionInputsResponse>, Status> {
        let request = request.into_inner();

        debug!(target: COMPONENT, ?request);

        let nullifiers = validate_nullifiers(&request.nullifiers)?;
        let account_id = request.account_id.ok_or(invalid_argument("Account_id missing"))?.id;

        let (account, nullifiers_blocks) = self
            .state
            .get_transaction_inputs(account_id, &nullifiers)
            .await
            .map_err(internal_error)?;

        Ok(Response::new(GetTransactionInputsResponse {
            account_state: Some(account.into()),
            nullifiers: convert(nullifiers_blocks),
        }))
    }

    // TESTING ENDPOINTS
    // --------------------------------------------------------------------------------------------

    /// Returns a list of all nullifiers
    async fn list_nullifiers(
        &self,
        _request: tonic::Request<ListNullifiersRequest>,
    ) -> Result<Response<ListNullifiersResponse>, Status> {
        let raw_nullifiers = self.state.list_nullifiers().await.map_err(internal_error)?;
        let nullifiers = raw_nullifiers
            .into_iter()
            .map(|(key, block_num)| NullifierLeaf {
                key: Some(Digest::from(key)),
                block_num,
            })
            .collect();
        Ok(Response::new(ListNullifiersResponse { nullifiers }))
    }

    /// Returns a list of all notes
    async fn list_notes(
        &self,
        _request: tonic::Request<ListNotesRequest>,
    ) -> Result<Response<ListNotesResponse>, Status> {
        let notes = self.state.list_notes().await.map_err(internal_error)?;
        Ok(Response::new(ListNotesResponse { notes }))
    }

    /// Returns a list of all accounts
    async fn list_accounts(
        &self,
        _request: tonic::Request<ListAccountsRequest>,
    ) -> Result<Response<ListAccountsResponse>, Status> {
        let accounts = self.state.list_accounts().await.map_err(internal_error)?;
        Ok(Response::new(ListAccountsResponse { accounts }))
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

#[instrument(target = "miden-store", skip_all, err)]
fn validate_nullifiers(nullifiers: &[Digest]) -> Result<Vec<RpoDigest>, Status> {
    nullifiers
        .iter()
        .map(|v| v.try_into())
        .collect::<Result<Vec<RpoDigest>, ParseError>>()
        .map_err(|_| invalid_argument("Digest field is not in the modulus range"))
}
