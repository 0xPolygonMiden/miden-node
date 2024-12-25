use miden_node_proto::{
    generated::{
        block_producer::api_client as block_producer_client,
        requests::{
            CheckNullifiersByPrefixRequest, CheckNullifiersRequest, GetAccountDetailsRequest,
            GetAccountProofsRequest, GetAccountStateDeltaRequest, GetBlockByNumberRequest,
            GetBlockHeaderByNumberRequest, GetNotesByIdRequest, SubmitProvenTransactionRequest,
            SyncNoteRequest, SyncStateRequest,
        },
        responses::{
            CheckNullifiersByPrefixResponse, CheckNullifiersResponse, GetAccountDetailsResponse,
            GetAccountProofsResponse, GetAccountStateDeltaResponse, GetBlockByNumberResponse,
            GetBlockHeaderByNumberResponse, GetNotesByIdResponse, SubmitProvenTransactionResponse,
            SyncNoteResponse, SyncStateResponse,
        },
        rpc::api_server,
        store::api_client as store_client,
    },
    try_convert,
};
use miden_objects::{
    accounts::AccountId, crypto::hash::rpo::RpoDigest, transaction::ProvenTransaction,
    utils::serde::Deserializable, Digest, MAX_NUM_FOREIGN_ACCOUNTS, MIN_PROOF_SECURITY_LEVEL,
};
use miden_tx::TransactionVerifier;
use tonic::{
    transport::{Channel, Error},
    Request, Response, Status,
};
use tracing::{debug, info, instrument};

use crate::{config::RpcConfig, COMPONENT};

// RPC API
// ================================================================================================

pub struct RpcApi {
    store: store_client::ApiClient<Channel>,
    block_producer: block_producer_client::ApiClient<Channel>,
}

impl RpcApi {
    pub(super) async fn from_config(config: &RpcConfig) -> Result<Self, Error> {
        let store = store_client::ApiClient::connect(config.store_url.clone()).await?;
        info!(target: COMPONENT, store_endpoint = config.store_url, "Store client initialized");

        let block_producer =
            block_producer_client::ApiClient::connect(config.block_producer_url.clone()).await?;
        info!(
            target: COMPONENT,
            block_producer_endpoint = config.block_producer_url,
            "Block producer client initialized",
        );

        Ok(Self { store, block_producer })
    }
}

#[tonic::async_trait]
impl api_server::Api for RpcApi {
    #[instrument(
        target = COMPONENT,
        name = "rpc:check_nullifiers",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn check_nullifiers(
        &self,
        request: Request<CheckNullifiersRequest>,
    ) -> Result<Response<CheckNullifiersResponse>, Status> {
        debug!(target: COMPONENT, request = ?request.get_ref());

        // validate all the nullifiers from the user request
        for nullifier in request.get_ref().nullifiers.iter() {
            let _: Digest = nullifier
                .try_into()
                .or(Err(Status::invalid_argument("Digest field is not in the modulus range")))?;
        }

        self.store.clone().check_nullifiers(request).await
    }

    #[instrument(
        target = COMPONENT,
        name = "rpc:check_nullifiers_by_prefix",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn check_nullifiers_by_prefix(
        &self,
        request: Request<CheckNullifiersByPrefixRequest>,
    ) -> Result<Response<CheckNullifiersByPrefixResponse>, Status> {
        debug!(target: COMPONENT, request = ?request.get_ref());

        self.store.clone().check_nullifiers_by_prefix(request).await
    }

    #[instrument(
        target = COMPONENT,
        name = "rpc:get_block_header_by_number",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_block_header_by_number(
        &self,
        request: Request<GetBlockHeaderByNumberRequest>,
    ) -> Result<Response<GetBlockHeaderByNumberResponse>, Status> {
        info!(target: COMPONENT, request = ?request.get_ref());

        self.store.clone().get_block_header_by_number(request).await
    }

    #[instrument(
        target = COMPONENT,
        name = "rpc:sync_state",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn sync_state(
        &self,
        request: Request<SyncStateRequest>,
    ) -> Result<Response<SyncStateResponse>, Status> {
        debug!(target: COMPONENT, request = ?request.get_ref());

        self.store.clone().sync_state(request).await
    }

    #[instrument(
        target = COMPONENT,
        name = "rpc:sync_notes",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn sync_notes(
        &self,
        request: Request<SyncNoteRequest>,
    ) -> Result<Response<SyncNoteResponse>, Status> {
        debug!(target: COMPONENT, request = ?request.get_ref());

        self.store.clone().sync_notes(request).await
    }

    #[instrument(
        target = COMPONENT,
        name = "rpc:get_notes_by_id",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_notes_by_id(
        &self,
        request: Request<GetNotesByIdRequest>,
    ) -> Result<Response<GetNotesByIdResponse>, Status> {
        debug!(target: COMPONENT, request = ?request.get_ref());

        // Validation checking for correct NoteId's
        let note_ids = request.get_ref().note_ids.clone();

        let _: Vec<RpoDigest> = try_convert(note_ids)
            .map_err(|err| Status::invalid_argument(format!("Invalid NoteId: {}", err)))?;

        self.store.clone().get_notes_by_id(request).await
    }

    #[instrument(target = COMPONENT, name = "rpc:submit_proven_transaction", skip_all, err)]
    async fn submit_proven_transaction(
        &self,
        request: Request<SubmitProvenTransactionRequest>,
    ) -> Result<Response<SubmitProvenTransactionResponse>, Status> {
        debug!(target: COMPONENT, request = ?request.get_ref());

        let request = request.into_inner();

        let tx = ProvenTransaction::read_from_bytes(&request.transaction)
            .map_err(|err| Status::invalid_argument(format!("Invalid transaction: {err}")))?;

        let tx_verifier = TransactionVerifier::new(MIN_PROOF_SECURITY_LEVEL);

        tx_verifier.verify(tx.clone()).map_err(|err| {
            Status::invalid_argument(format!("Invalid proof for transaction {}: {err}", tx.id()))
        })?;

        self.block_producer.clone().submit_proven_transaction(request).await
    }

    /// Returns details for public (public) account by id.
    #[instrument(
        target = COMPONENT,
        name = "rpc:get_account_details",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_account_details(
        &self,
        request: Request<GetAccountDetailsRequest>,
    ) -> std::result::Result<Response<GetAccountDetailsResponse>, Status> {
        debug!(target: COMPONENT, request = ?request.get_ref());

        // Validating account using conversion:
        let _account_id: AccountId = request
            .get_ref()
            .account_id
            .clone()
            .ok_or(Status::invalid_argument("account_id is missing"))?
            .try_into()
            .map_err(|err| Status::invalid_argument(format!("Invalid account id: {err}")))?;

        self.store.clone().get_account_details(request).await
    }

    #[instrument(
        target = COMPONENT,
        name = "rpc:get_block_by_number",
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

        self.store.clone().get_block_by_number(request).await
    }

    #[instrument(
        target = COMPONENT,
        name = "rpc:get_account_state_delta",
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

        self.store.clone().get_account_state_delta(request).await
    }

    #[instrument(
        target = COMPONENT,
        name = "rpc:get_account_proofs",
        skip_all,
        ret(level = "debug"),
        err
    )]
    async fn get_account_proofs(
        &self,
        request: Request<GetAccountProofsRequest>,
    ) -> Result<Response<GetAccountProofsResponse>, Status> {
        let request = request.into_inner();

        debug!(target: COMPONENT, ?request);

        if request.account_ids.len() > MAX_NUM_FOREIGN_ACCOUNTS as usize {
            return Err(Status::invalid_argument(format!(
                "Too many accounts requested: {}, limit: {MAX_NUM_FOREIGN_ACCOUNTS}",
                request.account_ids.len()
            )));
        }

        self.store.clone().get_account_proofs(request).await
    }
}
