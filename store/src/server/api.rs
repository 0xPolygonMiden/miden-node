use std::sync::Arc;

use miden_node_proto::{
    convert,
    errors::ConversionError,
    generated::{
        self,
        note::NoteSyncRecord,
        requests::{
            ApplyBlockRequest, CheckNullifiersRequest, GetBlockHeaderByNumberRequest,
            GetBlockInputsRequest, GetTransactionInputsRequest, ListAccountsRequest,
            ListNotesRequest, ListNullifiersRequest, SyncStateRequest,
        },
        responses::{
            AccountHashUpdate, AccountTransactionInputRecord, ApplyBlockResponse,
            CheckNullifiersResponse, GetBlockHeaderByNumberResponse, GetBlockInputsResponse,
            GetTransactionInputsResponse, ListAccountsResponse, ListNotesResponse,
            ListNullifiersResponse, NullifierTransactionInputRecord, NullifierUpdate,
            SyncStateResponse,
        },
        smt::SmtLeafEntry,
        store::api_server,
    },
    AccountState,
};
use miden_objects::{notes::Nullifier, BlockHeader, Felt, ZERO};
use tonic::{Response, Status};
use tracing::{debug, info, instrument};

use crate::{db::NoteCreated, state::State, types::AccountId, COMPONENT};

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
        target = "miden-store",
        name = "store:get_block_header_by_number",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_block_header_by_number(
        &self,
        request: tonic::Request<GetBlockHeaderByNumberRequest>,
    ) -> Result<Response<GetBlockHeaderByNumberResponse>, Status> {
        info!(target: COMPONENT, ?request);

        let block_num = request.into_inner().block_num;
        let block_header = self
            .state
            .get_block_header(block_num)
            .await
            .map_err(internal_error)?
            .map(Into::into);

        Ok(Response::new(GetBlockHeaderByNumberResponse { block_header }))
    }

    /// Returns info on whether the specified nullifiers have been consumed.
    ///
    /// This endpoint also returns Merkle authentication path for each requested nullifier which can
    /// be verified against the latest root of the nullifier database.
    #[instrument(
        target = "miden-store",
        name = "store:check_nullifiers",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn check_nullifiers(
        &self,
        request: tonic::Request<CheckNullifiersRequest>,
    ) -> Result<Response<CheckNullifiersResponse>, Status> {
        // Validate the nullifiers and convert them to Digest values. Stop on first error.
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
    #[instrument(
        target = "miden-store",
        name = "store:sync_state",
        skip_all,
        ret(level = "debug"),
        err
    )]
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

        let accounts = state
            .account_updates
            .into_iter()
            .map(|account_info| AccountHashUpdate {
                account_id: Some(account_info.account_id.into()),
                account_hash: Some(account_info.account_hash.into()),
                block_num: account_info.block_num,
            })
            .collect();

        let notes = state
            .notes
            .into_iter()
            .map(|note| NoteSyncRecord {
                note_index: note.note_created.absolute_note_index(),
                note_id: Some(note.note_created.note_id.into()),
                sender: Some(note.note_created.sender.into()),
                tag: note.note_created.tag,
                merkle_path: Some(note.merkle_path.into()),
            })
            .collect();

        let nullifiers = state
            .nullifiers
            .into_iter()
            .map(|nullifier_info| NullifierUpdate {
                nullifier: Some(nullifier_info.nullifier.into()),
                block_num: nullifier_info.block_num,
            })
            .collect();

        Ok(Response::new(SyncStateResponse {
            chain_tip: state.chain_tip,
            block_header: Some(state.block_header.into()),
            mmr_delta: Some(delta.into()),
            accounts,
            notes,
            nullifiers,
        }))
    }

    // BLOCK PRODUCER ENDPOINTS
    // --------------------------------------------------------------------------------------------

    /// Updates the local DB by inserting a new block header and the related data.
    #[instrument(
        target = "miden-store",
        name = "store:apply_block",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn apply_block(
        &self,
        request: tonic::Request<ApplyBlockRequest>,
    ) -> Result<tonic::Response<ApplyBlockResponse>, tonic::Status> {
        let request = request.into_inner();

        debug!(target: COMPONENT, ?request);
        let block_header: BlockHeader = request
            .block
            .ok_or(invalid_argument("Apply block missing block header"))?
            .try_into()
            .map_err(|err: ConversionError| Status::invalid_argument(err.to_string()))?;

        info!(target: COMPONENT, block_num = block_header.block_num(), block_hash = %block_header.hash());

        let nullifiers = validate_nullifiers(&request.nullifiers)?;
        let accounts = request
            .accounts
            .into_iter()
            .map(|account_update| {
                let account_state: AccountState = account_update
                    .try_into()
                    .map_err(|err: ConversionError| Status::invalid_argument(err.to_string()))?;
                Ok((
                    account_state.account_id.into(),
                    account_state
                        .account_hash
                        .ok_or(invalid_argument("Account update missing account hash"))?,
                ))
            })
            .collect::<Result<Vec<_>, Status>>()?;

        let notes = request
            .notes
            .into_iter()
            .map(|note| {
                Ok(NoteCreated {
                    batch_index: note.batch_index,
                    note_index: note.note_index,
                    note_id: note
                        .note_id
                        .ok_or(invalid_argument("Note missing id"))?
                        .try_into()
                        .map_err(|err: ConversionError| {
                            Status::invalid_argument(err.to_string())
                        })?,
                    sender: note.sender.ok_or(invalid_argument("Note missing sender"))?.into(),
                    tag: note.tag,
                })
            })
            .collect::<Result<Vec<_>, Status>>()?;

        let _ = self.state.apply_block(block_header, nullifiers, accounts, notes).await;

        Ok(Response::new(ApplyBlockResponse {}))
    }

    /// Returns data needed by the block producer to construct and prove the next block.
    #[instrument(
        target = "miden-store",
        name = "store:get_block_inputs",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_block_inputs(
        &self,
        request: tonic::Request<GetBlockInputsRequest>,
    ) -> Result<Response<GetBlockInputsResponse>, Status> {
        let request = request.into_inner();

        let nullifiers = validate_nullifiers(&request.nullifiers)?;
        let account_ids: Vec<AccountId> = request.account_ids.iter().map(|e| e.id).collect();

        let (latest, accumulator, account_states, nullifier_records) = self
            .state
            .get_block_inputs(&account_ids, &nullifiers)
            .await
            .map_err(internal_error)?;

        Ok(Response::new(GetBlockInputsResponse {
            block_header: Some(latest.into()),
            mmr_peaks: convert(accumulator.peaks()),
            account_states: convert(account_states),
            nullifiers: convert(nullifier_records),
        }))
    }

    #[instrument(
        target = "miden-store",
        name = "store:get_transaction_inputs",
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

        let tx_inputs = self.state.get_transaction_inputs(account_id, &nullifiers).await;

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
        }))
    }

    // TESTING ENDPOINTS
    // --------------------------------------------------------------------------------------------

    /// Returns a list of all nullifiers
    #[instrument(
        target = "miden-store",
        name = "store:list_nullifiers",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn list_nullifiers(
        &self,
        _request: tonic::Request<ListNullifiersRequest>,
    ) -> Result<Response<ListNullifiersResponse>, Status> {
        let raw_nullifiers = self.state.list_nullifiers().await.map_err(internal_error)?;
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
        target = "miden-store",
        name = "store:list_notes",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn list_notes(
        &self,
        _request: tonic::Request<ListNotesRequest>,
    ) -> Result<Response<ListNotesResponse>, Status> {
        let notes = self
            .state
            .list_notes()
            .await
            .map_err(internal_error)?
            .into_iter()
            .map(|note| generated::note::Note {
                block_num: note.block_num,
                note_index: note.note_created.note_index,
                note_id: Some(note.note_created.note_id.into()),
                sender: Some(note.note_created.sender.into()),
                tag: note.note_created.tag,
                merkle_path: Some(note.merkle_path.into()),
            })
            .collect();
        Ok(Response::new(ListNotesResponse { notes }))
    }

    /// Returns a list of all accounts
    #[instrument(
        target = "miden-store",
        name = "store:list_accounts",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn list_accounts(
        &self,
        _request: tonic::Request<ListAccountsRequest>,
    ) -> Result<Response<ListAccountsResponse>, Status> {
        let accounts = self
            .state
            .list_accounts()
            .await
            .map_err(internal_error)?
            .into_iter()
            .map(|account_info| generated::account::AccountInfo {
                account_id: Some(account_info.account_id.into()),
                account_hash: Some(account_info.account_hash.into()),
                block_num: account_info.block_num,
            })
            .collect();
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
fn validate_nullifiers(nullifiers: &[generated::digest::Digest]) -> Result<Vec<Nullifier>, Status> {
    nullifiers
        .iter()
        .cloned()
        .map(TryInto::try_into)
        .collect::<Result<_, ConversionError>>()
        .map_err(|_| invalid_argument("Digest field is not in the modulus range"))
}
