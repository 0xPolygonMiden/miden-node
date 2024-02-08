use async_trait::async_trait;
use miden_node_proto::{
    convert,
    domain::blocks::BlockInputs,
    generated::{
        account, digest,
        requests::{ApplyBlockRequest, GetBlockInputsRequest, GetTransactionInputsRequest},
        store::api_client as store_client,
    },
    TransactionInputs,
};
use miden_objects::{accounts::AccountId, Digest};
use tonic::transport::Channel;
use tracing::{debug, info, instrument};

pub use crate::errors::{ApplyBlockError, BlockInputsError, TxInputsError};
use crate::{block::Block, ProvenTransaction, COMPONENT};

// STORE TRAIT
// ================================================================================================

#[async_trait]
pub trait Store: ApplyBlock {
    /// TODO: add comments
    async fn get_tx_inputs(
        &self,
        proven_tx: &ProvenTransaction,
    ) -> Result<TransactionInputs, TxInputsError>;

    /// TODO: add comments
    async fn get_block_inputs(
        &self,
        updated_accounts: impl Iterator<Item = &AccountId> + Send,
        produced_nullifiers: impl Iterator<Item = &Digest> + Send,
    ) -> Result<BlockInputs, BlockInputsError>;
}

#[async_trait]
pub trait ApplyBlock: Send + Sync + 'static {
    async fn apply_block(
        &self,
        block: Block,
    ) -> Result<(), ApplyBlockError>;
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

#[async_trait]
impl ApplyBlock for DefaultStore {
    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(target = "miden-block-producer", skip_all, err)]
    async fn apply_block(
        &self,
        block: Block,
    ) -> Result<(), ApplyBlockError> {
        let request = tonic::Request::new(ApplyBlockRequest {
            block: Some(block.header.into()),
            accounts: convert(block.updated_accounts),
            nullifiers: convert(block.produced_nullifiers),
            notes: convert(block.created_notes),
        });

        let _ = self
            .store
            .clone()
            .apply_block(request)
            .await
            .map_err(|status| ApplyBlockError::GrpcClientError(status.message().to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl Store for DefaultStore {
    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
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
                .map(|nullifier| (*nullifier).into())
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

        if tx_inputs.account_state.account_id != proven_tx.account_id() {
            return Err(TxInputsError::MalformedResponse(format!(
                "incorrect account id returned from store. Got: {}, expected: {}",
                tx_inputs.account_state.account_id,
                proven_tx.account_id()
            )));
        }

        debug!(target: COMPONENT, %tx_inputs);

        Ok(tx_inputs)
    }

    async fn get_block_inputs(
        &self,
        updated_accounts: impl Iterator<Item = &AccountId> + Send,
        produced_nullifiers: impl Iterator<Item = &Digest> + Send,
    ) -> Result<BlockInputs, BlockInputsError> {
        let request = tonic::Request::new(GetBlockInputsRequest {
            account_ids: updated_accounts
                .map(|&account_id| account::AccountId::from(account_id))
                .collect(),
            nullifiers: produced_nullifiers.map(digest::Digest::from).collect(),
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
}
