use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
};

use async_trait::async_trait;
use miden_node_proto::{
    convert,
    errors::{ConversionError, MissingFieldHelper},
    generated::{
        digest,
        requests::{
            ApplyBlockRequest, GetBlockInputsRequest, GetMissingNotesRequest,
            GetTransactionInputsRequest,
        },
        responses::{GetTransactionInputsResponse, NullifierTransactionInputRecord},
        store::api_client as store_client,
    },
    AccountState,
};
use miden_node_utils::formatting::{format_map, format_opt};
use miden_objects::{
    accounts::AccountId,
    block::Block,
    notes::{NoteId, Nullifier},
    utils::Serializable,
    Digest,
};
use miden_processor::crypto::RpoDigest;
use tonic::transport::Channel;
use tracing::{debug, info, instrument};

pub use crate::errors::{ApplyBlockError, BlockInputsError, TxInputsError};
use crate::{block::BlockInputs, errors::GetMissingNotesError, ProvenTransaction, COMPONENT};

// STORE TRAIT
// ================================================================================================

#[async_trait]
pub trait Store: ApplyBlock {
    /// Return information needed from the store to verify a given proven transaction.
    async fn get_tx_inputs(
        &self,
        proven_tx: &ProvenTransaction,
    ) -> Result<TransactionInputs, TxInputsError>;

    /// Return information needed from the store to build a block.
    async fn get_block_inputs(
        &self,
        updated_accounts: impl Iterator<Item = AccountId> + Send,
        produced_nullifiers: impl Iterator<Item = &Nullifier> + Send,
        notes: impl Iterator<Item = &NoteId> + Send,
    ) -> Result<BlockInputs, BlockInputsError>;

    async fn get_missing_notes(
        &self,
        notes: &[NoteId],
    ) -> Result<Vec<NoteId>, GetMissingNotesError>;
}

#[async_trait]
pub trait ApplyBlock: Send + Sync + 'static {
    async fn apply_block(&self, block: &Block) -> Result<(), ApplyBlockError>;
}

// TRANSACTION INPUTS
// ================================================================================================

/// Information needed from the store to verify a transaction.
#[derive(Debug)]
pub struct TransactionInputs {
    /// Account ID
    pub account_id: AccountId,
    /// The account hash in the store corresponding to tx's account ID
    pub account_hash: Option<Digest>,
    /// Maps each consumed notes' nullifier to block number, where the note is consumed
    /// (`zero` means, that note isn't consumed yet)
    pub nullifiers: BTreeMap<Nullifier, u32>,
    /// List of notes that were not found in the store
    pub missing_notes: Vec<NoteId>,
}

impl Display for TransactionInputs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ account_id: {}, account_hash: {}, nullifiers: {} }}",
            self.account_id,
            format_opt(self.account_hash.as_ref()),
            format_map(&self.nullifiers)
        ))
    }
}

impl TryFrom<GetTransactionInputsResponse> for TransactionInputs {
    type Error = ConversionError;

    fn try_from(response: GetTransactionInputsResponse) -> Result<Self, Self::Error> {
        let AccountState { account_id, account_hash } = response
            .account_state
            .ok_or(GetTransactionInputsResponse::missing_field(stringify!(account_state)))?
            .try_into()?;

        let mut nullifiers = BTreeMap::new();
        for nullifier_record in response.nullifiers {
            let nullifier = nullifier_record
                .nullifier
                .ok_or(NullifierTransactionInputRecord::missing_field(stringify!(nullifier)))?
                .try_into()?;

            nullifiers.insert(nullifier, nullifier_record.block_num);
        }

        let missing_notes = response
            .missing_notes
            .into_iter()
            .map(|digest| Ok(RpoDigest::try_from(digest)?.into()))
            .collect::<Result<Vec<_>, ConversionError>>()?;

        Ok(Self {
            account_id,
            account_hash,
            nullifiers,
            missing_notes,
        })
    }
}

// DEFAULT STORE IMPLEMENTATION
// ================================================================================================

pub struct DefaultStore {
    store: store_client::ApiClient<Channel>,
}

impl DefaultStore {
    /// TODO: this should probably take store connection string and create a connection internally
    pub fn new(store: store_client::ApiClient<Channel>) -> Self {
        Self { store }
    }
}

// FIXME: remove the allow when the upstream clippy issue is fixed:
// https://github.com/rust-lang/rust-clippy/issues/12281
#[allow(clippy::blocks_in_conditions)]
#[async_trait]
impl ApplyBlock for DefaultStore {
    #[instrument(target = "miden-block-producer", skip_all, err)]
    async fn apply_block(&self, block: &Block) -> Result<(), ApplyBlockError> {
        let request = tonic::Request::new(ApplyBlockRequest { block: block.to_bytes() });

        let _ = self
            .store
            .clone()
            .apply_block(request)
            .await
            .map_err(|status| ApplyBlockError::GrpcClientError(status.message().to_string()))?;

        Ok(())
    }
}

// FIXME: remove the allow when the upstream clippy issue is fixed:
// https://github.com/rust-lang/rust-clippy/issues/12281
#[allow(clippy::blocks_in_conditions)]
#[async_trait]
impl Store for DefaultStore {
    #[instrument(target = "miden-block-producer", skip_all, err)]
    async fn get_tx_inputs(
        &self,
        proven_tx: &ProvenTransaction,
    ) -> Result<TransactionInputs, TxInputsError> {
        let message = GetTransactionInputsRequest {
            account_id: Some(proven_tx.account_id().into()),
            nullifiers: proven_tx
                .input_notes()
                .iter()
                .map(|note| note.nullifier().into())
                .collect(),
            notes: proven_tx
                .input_notes()
                .iter()
                .filter_map(|note| note.note_id().map(|id| id.into()))
                .collect(),
        };

        info!(target: COMPONENT, tx_id = %proven_tx.id().to_hex());
        debug!(target: COMPONENT, ?message);

        let request = tonic::Request::new(message);
        let response = self
            .store
            .clone()
            .get_transaction_inputs(request)
            .await
            .map_err(|status| TxInputsError::GrpcClientError(status.message().to_string()))?
            .into_inner();

        debug!(target: COMPONENT, ?response);

        let tx_inputs: TransactionInputs = response.try_into()?;

        if tx_inputs.account_id != proven_tx.account_id() {
            return Err(TxInputsError::MalformedResponse(format!(
                "incorrect account id returned from store. Got: {}, expected: {}",
                tx_inputs.account_id,
                proven_tx.account_id()
            )));
        }

        debug!(target: COMPONENT, %tx_inputs);

        Ok(tx_inputs)
    }

    async fn get_block_inputs(
        &self,
        updated_accounts: impl Iterator<Item = AccountId> + Send,
        produced_nullifiers: impl Iterator<Item = &Nullifier> + Send,
        notes: impl Iterator<Item = &NoteId> + Send,
    ) -> Result<BlockInputs, BlockInputsError> {
        let request = tonic::Request::new(GetBlockInputsRequest {
            account_ids: updated_accounts.map(Into::into).collect(),
            nullifiers: produced_nullifiers.map(digest::Digest::from).collect(),
            notes: notes.map(digest::Digest::from).collect(),
        });

        let store_response = self
            .store
            .clone()
            .get_block_inputs(request)
            .await
            .map_err(|err| BlockInputsError::GrpcClientError(err.message().to_string()))?
            .into_inner();

        Ok(store_response.try_into()?)
    }

    #[instrument(target = "miden-block-producer", skip_all, err)]
    async fn get_missing_notes(
        &self,
        notes: &[NoteId],
    ) -> Result<Vec<NoteId>, GetMissingNotesError> {
        let message = GetMissingNotesRequest { notes: convert(notes) };

        debug!(target: COMPONENT, ?message);

        let request = tonic::Request::new(message);
        let response = self
            .store
            .clone()
            .get_missing_notes(request)
            .await
            .map_err(|status| GetMissingNotesError::GrpcClientError(status.message().to_string()))?
            .into_inner();

        debug!(target: COMPONENT, ?response);

        let missing_notes = response
            .missing_notes
            .into_iter()
            .map(|digest| Ok(RpoDigest::try_from(digest)?.into()))
            .collect::<Result<Vec<_>, ConversionError>>()?;

        Ok(missing_notes)
    }
}
