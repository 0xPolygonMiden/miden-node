use std::{collections::BTreeSet, sync::Arc};

use miden_node_proto::{
    convert,
    domain::{accounts::AccountInfo, notes::NoteAuthenticationInfo},
    errors::ConversionError,
    generated::{
        self,
        account::AccountSummary,
        note::NoteAuthenticationInfo as NoteAuthenticationInfoProto,
        requests::{
            ApplyBlockRequest, CheckNullifiersByPrefixRequest, CheckNullifiersRequest,
            GetAccountDetailsRequest, GetAccountProofsRequest, GetAccountStateDeltaRequest,
            GetBlockByNumberRequest, GetBlockHeaderByNumberRequest, GetBlockInputsRequest,
            GetNoteAuthenticationInfoRequest, GetNotesByIdRequest, GetTransactionInputsRequest,
            ListAccountsRequest, ListNotesRequest, ListNullifiersRequest, SyncNoteRequest,
            SyncStateRequest,
        },
        responses::{
            AccountTransactionInputRecord, ApplyBlockResponse, CheckNullifiersByPrefixResponse,
            CheckNullifiersResponse, GetAccountDetailsResponse, GetAccountProofsResponse,
            GetAccountStateDeltaResponse, GetBlockByNumberResponse, GetBlockHeaderByNumberResponse,
            GetBlockInputsResponse, GetNoteAuthenticationInfoResponse, GetNotesByIdResponse,
            GetTransactionInputsResponse, ListAccountsResponse, ListNotesResponse,
            ListNullifiersResponse, NullifierTransactionInputRecord, NullifierUpdate,
            SyncNoteResponse, SyncStateResponse,
        },
        smt::SmtLeafEntry,
        store::api_server,
        transaction::TransactionSummary,
    },
    try_convert,
};
use miden_objects::{
    accounts::AccountId,
    block::Block,
    crypto::hash::rpo::RpoDigest,
    notes::{NoteId, Nullifier},
    utils::{Deserializable, Serializable},
    Felt, ZERO,
};
use tonic::{Request, Response, Status};
use tracing::{debug, info, instrument};

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
    #[instrument(
        target = COMPONENT,
        name = "store:get_block_header_by_number",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_block_header_by_number(
        &self,
        request: Request<GetBlockHeaderByNumberRequest>,
    ) -> Result<Response<GetBlockHeaderByNumberResponse>, Status> {
        info!(target: COMPONENT, ?request);
        let request = request.into_inner();

        let block_num = request.block_num;
        let (block_header, mmr_proof) = self
            .state
            .get_block_header(block_num, request.include_mmr_proof.unwrap_or(false))
            .await
            .map_err(internal_error)?;

        Ok(Response::new(GetBlockHeaderByNumberResponse {
            block_header: block_header.map(Into::into),
            chain_length: mmr_proof.as_ref().map(|p| p.forest as u32),
            mmr_path: mmr_proof.map(|p| Into::into(&p.merkle_path)),
        }))
    }

    /// Returns info on whether the specified nullifiers have been consumed.
    ///
    /// This endpoint also returns Merkle authentication path for each requested nullifier which can
    /// be verified against the latest root of the nullifier database.
    #[instrument(
        target = COMPONENT,
        name = "store:check_nullifiers",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn check_nullifiers(
        &self,
        request: Request<CheckNullifiersRequest>,
    ) -> Result<Response<CheckNullifiersResponse>, Status> {
        // Validate the nullifiers and convert them to Digest values. Stop on first error.
        let request = request.into_inner();
        let nullifiers = validate_nullifiers(&request.nullifiers)?;

        // Query the state for the request's nullifiers
        let proofs = self.state.check_nullifiers(&nullifiers).await;

        Ok(Response::new(CheckNullifiersResponse { proofs: convert(proofs) }))
    }

    /// Returns nullifiers that match the specified prefixes and have been consumed.
    ///
    /// Currently the only supported prefix length is 16 bits.
    #[instrument(
        target = COMPONENT,
        name = "store:check_nullifiers_by_prefix",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn check_nullifiers_by_prefix(
        &self,
        request: Request<CheckNullifiersByPrefixRequest>,
    ) -> Result<Response<CheckNullifiersByPrefixResponse>, Status> {
        let request = request.into_inner();

        if request.prefix_len != 16 {
            return Err(Status::invalid_argument("Only 16-bit prefixes are supported"));
        }

        let nullifiers = self
            .state
            .check_nullifiers_by_prefix(request.prefix_len, request.nullifiers)
            .await?
            .into_iter()
            .map(|nullifier_info| NullifierUpdate {
                nullifier: Some(nullifier_info.nullifier.into()),
                block_num: nullifier_info.block_num,
            })
            .collect();

        Ok(Response::new(CheckNullifiersByPrefixResponse { nullifiers }))
    }

    /// Returns info which can be used by the client to sync up to the latest state of the chain
    /// for the objects the client is interested in.
    #[instrument(
        target = COMPONENT,
        name = "store:sync_state",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn sync_state(
        &self,
        request: Request<SyncStateRequest>,
    ) -> Result<Response<SyncStateResponse>, Status> {
        let request = request.into_inner();

        let account_ids: Vec<AccountId> = read_account_ids(&request.account_ids)?;

        let (state, delta) = self
            .state
            .sync_state(request.block_num, account_ids, request.note_tags, request.nullifiers)
            .await
            .map_err(internal_error)?;

        let accounts = state
            .account_updates
            .into_iter()
            .map(|account_info| AccountSummary {
                account_id: Some(account_info.account_id.into()),
                account_hash: Some(account_info.account_hash.into()),
                block_num: account_info.block_num,
            })
            .collect();

        let transactions = state
            .transactions
            .into_iter()
            .map(|transaction_summary| TransactionSummary {
                account_id: Some(transaction_summary.account_id.into()),
                block_num: transaction_summary.block_num,
                transaction_id: Some(transaction_summary.transaction_id.into()),
            })
            .collect();

        let notes = state.notes.into_iter().map(Into::into).collect();

        let nullifiers = state
            .nullifiers
            .into_iter()
            .map(|nullifier_info| NullifierUpdate {
                nullifier: Some(nullifier_info.nullifier.into()),
                block_num: nullifier_info.block_num,
            })
            .collect();

        Ok(Response::new(SyncStateResponse {
            chain_tip: self.state.latest_block_num().await,
            block_header: Some(state.block_header.into()),
            mmr_delta: Some(delta.into()),
            accounts,
            transactions,
            notes,
            nullifiers,
        }))
    }

    /// Returns info which can be used by the client to sync note state.
    #[instrument(
        target = COMPONENT,
        name = "store:sync_notes",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn sync_notes(
        &self,
        request: Request<SyncNoteRequest>,
    ) -> Result<Response<SyncNoteResponse>, Status> {
        let request = request.into_inner();

        let (state, mmr_proof) = self
            .state
            .sync_notes(request.block_num, request.note_tags)
            .await
            .map_err(internal_error)?;

        let notes = state.notes.into_iter().map(Into::into).collect();

        Ok(Response::new(SyncNoteResponse {
            chain_tip: self.state.latest_block_num().await,
            block_header: Some(state.block_header.into()),
            mmr_path: Some((&mmr_proof.merkle_path).into()),
            notes,
        }))
    }

    /// Returns a list of Note's for the specified NoteId's.
    ///
    /// If the list is empty or no Note matched the requested NoteId and empty list is returned.
    #[instrument(
        target = COMPONENT,
        name = "store:get_notes_by_id",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_notes_by_id(
        &self,
        request: Request<GetNotesByIdRequest>,
    ) -> Result<Response<GetNotesByIdResponse>, Status> {
        info!(target: COMPONENT, ?request);

        let note_ids = request.into_inner().note_ids;

        let note_ids: Vec<RpoDigest> = try_convert(note_ids)
            .map_err(|err| Status::invalid_argument(format!("Invalid NoteId: {}", err)))?;

        let note_ids: Vec<NoteId> = note_ids.into_iter().map(From::from).collect();

        let notes = self
            .state
            .get_notes_by_id(note_ids)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();

        Ok(Response::new(GetNotesByIdResponse { notes }))
    }

    /// Returns a list of Note inclusion proofs for the specified NoteId's.
    #[instrument(
        target = COMPONENT,
        name = "store:get_note_inclusion_proofs",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_note_authentication_info(
        &self,
        request: Request<GetNoteAuthenticationInfoRequest>,
    ) -> Result<Response<GetNoteAuthenticationInfoResponse>, Status> {
        info!(target: COMPONENT, ?request);

        let note_ids = request.into_inner().note_ids;

        let note_ids: Vec<RpoDigest> = try_convert(note_ids)
            .map_err(|err| Status::invalid_argument(format!("Invalid NoteId: {}", err)))?;

        let note_ids = note_ids.into_iter().map(From::from).collect();

        let NoteAuthenticationInfo { block_proofs, note_proofs } = self
            .state
            .get_note_authentication_info(note_ids)
            .await
            .map_err(internal_error)?;

        // Massage into shape required by protobuf
        let note_proofs = note_proofs.iter().map(Into::into).collect();
        let block_proofs = block_proofs.into_iter().map(Into::into).collect();

        Ok(Response::new(GetNoteAuthenticationInfoResponse {
            proofs: Some(NoteAuthenticationInfoProto { note_proofs, block_proofs }),
        }))
    }

    /// Returns details for public (public) account by id.
    #[instrument(
        target = COMPONENT,
        name = "store:get_account_details",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_account_details(
        &self,
        request: Request<GetAccountDetailsRequest>,
    ) -> Result<Response<GetAccountDetailsResponse>, Status> {
        let request = request.into_inner();
        let account_id = read_account_id(request.account_id)?;
        let account_info: AccountInfo = self.state.get_account_details(account_id).await?;

        Ok(Response::new(GetAccountDetailsResponse {
            details: Some((&account_info).into()),
        }))
    }

    // BLOCK PRODUCER ENDPOINTS
    // --------------------------------------------------------------------------------------------

    /// Updates the local DB by inserting a new block header and the related data.
    #[instrument(
        target = COMPONENT,
        name = "store:apply_block",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn apply_block(
        &self,
        request: Request<ApplyBlockRequest>,
    ) -> Result<Response<ApplyBlockResponse>, Status> {
        let request = request.into_inner();

        debug!(target: COMPONENT, ?request);

        let block = Block::read_from_bytes(&request.block).map_err(|err| {
            Status::invalid_argument(format!("Block deserialization error: {err}"))
        })?;

        let block_num = block.header().block_num();

        info!(
            target: COMPONENT,
            block_num,
            block_hash = %block.hash(),
            account_count = block.updated_accounts().len(),
            note_count = block.notes().count(),
            nullifier_count = block.nullifiers().len(),
        );

        self.state.apply_block(block).await?;

        Ok(Response::new(ApplyBlockResponse {}))
    }

    /// Returns data needed by the block producer to construct and prove the next block.
    #[instrument(
        target = COMPONENT,
        name = "store:get_block_inputs",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_block_inputs(
        &self,
        request: Request<GetBlockInputsRequest>,
    ) -> Result<Response<GetBlockInputsResponse>, Status> {
        let request = request.into_inner();

        let nullifiers = validate_nullifiers(&request.nullifiers)?;
        let account_ids = read_account_ids(&request.account_ids)?;
        let unauthenticated_notes = validate_notes(&request.unauthenticated_notes)?;
        let unauthenticated_notes = unauthenticated_notes.into_iter().collect();

        self.state
            .get_block_inputs(&account_ids, &nullifiers, unauthenticated_notes)
            .await
            .map(Into::into)
            .map(Response::new)
            .map_err(internal_error)
    }

    #[instrument(
        target = COMPONENT,
        name = "store:get_transaction_inputs",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_transaction_inputs(
        &self,
        request: Request<GetTransactionInputsRequest>,
    ) -> Result<Response<GetTransactionInputsResponse>, Status> {
        let request = request.into_inner();

        debug!(target: COMPONENT, ?request);

        let account_id = read_account_id(request.account_id)?;
        let nullifiers = validate_nullifiers(&request.nullifiers)?;
        let unauthenticated_notes = validate_notes(&request.unauthenticated_notes)?;

        let tx_inputs = self
            .state
            .get_transaction_inputs(account_id, &nullifiers, unauthenticated_notes)
            .await?;

        let block_height = self.state.latest_block_num().await;

        Ok(Response::new(GetTransactionInputsResponse {
            account_state: Some(AccountTransactionInputRecord {
                account_id: Some(account_id.into()),
                account_hash: Some(tx_inputs.account_hash.into()),
            }),
            nullifiers: tx_inputs
                .nullifiers
                .into_iter()
                .map(|nullifier| NullifierTransactionInputRecord {
                    nullifier: Some(nullifier.nullifier.into()),
                    block_num: nullifier.block_num,
                })
                .collect(),
            found_unauthenticated_notes: tx_inputs
                .found_unauthenticated_notes
                .into_iter()
                .map(Into::into)
                .collect(),
            block_height,
        }))
    }

    #[instrument(
        target = COMPONENT,
        name = "store:get_block_by_number",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_block_by_number(
        &self,
        request: Request<GetBlockByNumberRequest>,
    ) -> Result<Response<GetBlockByNumberResponse>, Status> {
        let request = request.into_inner();

        debug!(target: COMPONENT, ?request);

        let block = self.state.load_block(request.block_num).await?;

        Ok(Response::new(GetBlockByNumberResponse { block }))
    }

    #[instrument(
        target = COMPONENT,
        name = "store:get_account_proofs",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_account_proofs(
        &self,
        request: Request<GetAccountProofsRequest>,
    ) -> Result<Response<GetAccountProofsResponse>, Status> {
        let request = request.into_inner();
        if request.account_ids.len() < request.code_commitments.len() {
            return Err(Status::invalid_argument(
                "The number of code commitments should not exceed the number of requested accounts.",
            ));
        }

        debug!(target: COMPONENT, ?request);

        let include_headers = request.include_headers.unwrap_or_default();
        let account_ids: Vec<AccountId> = read_account_ids(&request.account_ids)?;
        let request_code_commitments: BTreeSet<RpoDigest> = try_convert(request.code_commitments)
            .map_err(|err| {
            Status::invalid_argument(format!("Invalid code commitment: {}", err))
        })?;

        let (block_num, infos) = self
            .state
            .get_account_proofs(account_ids, request_code_commitments, include_headers)
            .await?;

        Ok(Response::new(GetAccountProofsResponse {
            block_num,
            account_proofs: infos.into_iter().map(Into::into).collect(),
        }))
    }

    #[instrument(
        target = COMPONENT,
        name = "store:get_account_state_delta",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_account_state_delta(
        &self,
        request: Request<GetAccountStateDeltaRequest>,
    ) -> Result<Response<GetAccountStateDeltaResponse>, Status> {
        let request = request.into_inner();

        debug!(target: COMPONENT, ?request);

        let account_id = read_account_id(request.account_id)?;
        let delta = self
            .state
            .get_account_state_delta(account_id, request.from_block_num, request.to_block_num)
            .await?
            .map(|delta| delta.to_bytes());

        Ok(Response::new(GetAccountStateDeltaResponse { delta }))
    }

    // TESTING ENDPOINTS
    // --------------------------------------------------------------------------------------------

    /// Returns a list of all nullifiers
    #[instrument(
        target = COMPONENT,
        name = "store:list_nullifiers",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn list_nullifiers(
        &self,
        _request: Request<ListNullifiersRequest>,
    ) -> Result<Response<ListNullifiersResponse>, Status> {
        let raw_nullifiers = self.state.list_nullifiers().await?;
        let nullifiers = raw_nullifiers
            .into_iter()
            .map(|(key, block_num)| SmtLeafEntry {
                key: Some(key.into()),
                value: Some([Felt::from(block_num), ZERO, ZERO, ZERO].into()),
            })
            .collect();
        Ok(Response::new(ListNullifiersResponse { nullifiers }))
    }

    /// Returns a list of all notes
    #[instrument(
        target = COMPONENT,
        name = "store:list_notes",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn list_notes(
        &self,
        _request: Request<ListNotesRequest>,
    ) -> Result<Response<ListNotesResponse>, Status> {
        let notes = self.state.list_notes().await?.into_iter().map(Into::into).collect();
        Ok(Response::new(ListNotesResponse { notes }))
    }

    /// Returns a list of all accounts
    #[instrument(
        target = COMPONENT,
        name = "store:list_accounts",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn list_accounts(
        &self,
        _request: Request<ListAccountsRequest>,
    ) -> Result<Response<ListAccountsResponse>, Status> {
        let accounts = self.state.list_accounts().await?.iter().map(Into::into).collect();
        Ok(Response::new(ListAccountsResponse { accounts }))
    }
}

// UTILITIES
// ================================================================================================

/// Formats an "Internal error" error
fn internal_error<E: core::fmt::Display>(err: E) -> Status {
    Status::internal(err.to_string())
}

/// Formats an "Invalid argument" error
fn invalid_argument<E: core::fmt::Display>(err: E) -> Status {
    Status::invalid_argument(err.to_string())
}

fn read_account_id(id: Option<generated::account::AccountId>) -> Result<AccountId, Status> {
    id.ok_or(invalid_argument("missing account ID"))?
        .try_into()
        .map_err(|err| invalid_argument(format!("invalid account ID: {}", err)))
}

#[instrument(target = COMPONENT, skip_all, err)]
fn read_account_ids(
    account_ids: &[generated::account::AccountId],
) -> Result<Vec<AccountId>, Status> {
    account_ids
        .iter()
        .cloned()
        .map(AccountId::try_from)
        .collect::<Result<_, ConversionError>>()
        .map_err(|_| invalid_argument("Byte array is not a valid AccountId"))
}

#[instrument(target = COMPONENT, skip_all, err)]
fn validate_nullifiers(nullifiers: &[generated::digest::Digest]) -> Result<Vec<Nullifier>, Status> {
    nullifiers
        .iter()
        .cloned()
        .map(TryInto::try_into)
        .collect::<Result<_, ConversionError>>()
        .map_err(|_| invalid_argument("Digest field is not in the modulus range"))
}

#[instrument(target = COMPONENT, skip_all, err)]
fn validate_notes(notes: &[generated::digest::Digest]) -> Result<Vec<NoteId>, Status> {
    notes
        .iter()
        .map(|digest| Ok(RpoDigest::try_from(digest)?.into()))
        .collect::<Result<_, ConversionError>>()
        .map_err(|_| invalid_argument("Digest field is not in the modulus range"))
}
