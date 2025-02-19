use miden_node_proto::errors::ConversionError;
use miden_node_utils::formatting::format_opt;
use miden_objects::{
    account::AccountId,
    block::BlockNumber,
    crypto::merkle::MerkleError,
    note::{NoteId, Nullifier},
    transaction::TransactionId,
    AccountDeltaError, Digest, ProposedBatchError,
};
use miden_processor::ExecutionError;
use miden_tx_batch_prover::errors::ProvenBatchError;
use thiserror::Error;
use tokio::task::JoinError;

// Block-producer errors
// =================================================================================================

#[derive(Debug, Error)]
pub enum BlockProducerError {
    /// A block-producer task completed although it should have ran indefinitely.
    #[error("task {task} completed unexpectedly")]
    TaskFailedSuccesfully { task: &'static str },

    /// A block-producer task panic'd.
    #[error("error joining {task} task")]
    JoinError { task: &'static str, source: JoinError },

    /// A block-producer task reported a transport error.
    #[error("task {task} had a transport error")]
    TonicTransportError {
        task: &'static str,
        source: tonic::transport::Error,
    },
}

// Transaction verification errors
// =================================================================================================

#[derive(Debug, Error)]
pub enum VerifyTxError {
    /// Another transaction already consumed the notes with given nullifiers
    #[error(
        "input notes with given nullifiers were already consumed by another transaction: {0:?}"
    )]
    InputNotesAlreadyConsumed(Vec<Nullifier>),

    /// Unauthenticated transaction notes were not found in the store or in outputs of in-flight
    /// transactions
    #[error(
        "unauthenticated transaction notes were not found in the store or in outputs of in-flight transactions: {0:?}"
    )]
    UnauthenticatedNotesNotFound(Vec<NoteId>),

    #[error("output note IDs already used: {0:?}")]
    OutputNotesAlreadyExist(Vec<NoteId>),

    /// The account's initial hash did not match the current account's hash
    #[error("incorrect account's initial hash ({tx_initial_account_hash}, current: {})", format_opt(.current_account_hash.as_ref()))]
    IncorrectAccountInitialHash {
        tx_initial_account_hash: Digest,
        current_account_hash: Option<Digest>,
    },

    /// Failed to retrieve transaction inputs from the store
    ///
    /// TODO: Make this an "internal error". Q: Should we have a single `InternalError` enum for
    /// all internal errors that can occur across the system?
    #[error("failed to retrieve transaction inputs from the store")]
    StoreConnectionFailed(#[from] StoreError),

    /// Failed to verify the transaction execution proof
    #[error("invalid transaction proof error for transaction: {0}")]
    InvalidTransactionProof(TransactionId),
}

// Transaction adding errors
// =================================================================================================

#[derive(Debug, Error)]
pub enum AddTransactionError {
    #[error("transaction verification failed")]
    VerificationFailed(#[from] VerifyTxError),

    #[error("transaction input data from block {input_block} is rejected as stale because it is older than the limit of {stale_limit}")]
    StaleInputs {
        input_block: BlockNumber,
        stale_limit: BlockNumber,
    },

    #[error("transaction deserialization failed")]
    TransactionDeserializationFailed(#[source] miden_objects::utils::DeserializationError),

    #[error(
        "transaction expired at block height {expired_at} but the block height limit was {limit}"
    )]
    Expired {
        expired_at: BlockNumber,
        limit: BlockNumber,
    },
}

impl From<AddTransactionError> for tonic::Status {
    fn from(value: AddTransactionError) -> Self {
        match value {
            AddTransactionError::VerificationFailed(
                VerifyTxError::InputNotesAlreadyConsumed(_)
                | VerifyTxError::UnauthenticatedNotesNotFound(_)
                | VerifyTxError::OutputNotesAlreadyExist(_)
                | VerifyTxError::IncorrectAccountInitialHash { .. }
                | VerifyTxError::InvalidTransactionProof(_),
            )
            | AddTransactionError::Expired { .. }
            | AddTransactionError::TransactionDeserializationFailed(_) => {
                Self::invalid_argument(value.to_string())
            },

            // Internal errors which should not be communicated to the user.
            AddTransactionError::VerificationFailed(VerifyTxError::StoreConnectionFailed(_))
            | AddTransactionError::StaleInputs { .. } => Self::internal("Internal error"),
        }
    }
}

// Batch building errors
// =================================================================================================

/// Error encountered while building a batch.
#[derive(Debug, Error)]
pub enum BuildBatchError {
    /// We sometimes randomly inject errors into the batch building process to test our failure
    /// responses.
    #[error("nothing actually went wrong, failure was injected on purpose")]
    InjectedFailure,

    #[error("batch proving task panic'd")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("failed to fetch batch inputs from store")]
    FetchBatchInputsFailed(#[source] StoreError),

    #[error("failed to build proposed transaction batch")]
    ProposeBatchError(#[source] ProposedBatchError),

    #[error("failed to prove proposed transaction batch")]
    ProveBatchError(#[source] ProvenBatchError),
}

// Block prover errors
// =================================================================================================

#[derive(Debug, Error)]
pub enum BlockProverError {
    #[error("received invalid merkle path")]
    InvalidMerklePaths(#[source] MerkleError),
    #[error("program execution failed")]
    ProgramExecutionFailed(#[source] ExecutionError),
    #[error("failed to retrieve {0} root from stack outputs")]
    InvalidRootOutput(&'static str),
}

// Block building errors
// =================================================================================================

#[derive(Debug, Error)]
pub enum BuildBlockError {
    #[error("failed to compute new block")]
    BlockProverFailed(#[from] BlockProverError),
    #[error("failed to apply block to store")]
    StoreApplyBlockFailed(#[source] StoreError),
    #[error("failed to get block inputs from store")]
    GetBlockInputsFailed(#[source] StoreError),
    #[error("block inputs from store did not contain data for account {0}")]
    MissingAccountInput(AccountId),
    #[error("block inputs from store contained extra data for accounts {0:?}")]
    ExtraStoreData(Vec<AccountId>),
    #[error("account {0} with state {1} cannot transaction to remaining states {2:?}")]
    InconsistentAccountStateTransition(AccountId, Digest, Vec<Digest>),
    #[error(
        "block inputs from store and transaction batches produced different nullifiers: {0:?}"
    )]
    InconsistentNullifiers(Vec<Nullifier>),
    #[error("unauthenticated transaction notes not found in the store or in outputs of other transactions in the block: {0:?}")]
    UnauthenticatedNotesNotFound(Vec<NoteId>),
    #[error("failed to merge transaction delta into account {account_id}")]
    AccountUpdateError {
        account_id: AccountId,
        source: AccountDeltaError,
    },
    // TODO: Check if needed.
    // #[error("block construction failed")]
    // BlockConstructionError,
    /// We sometimes randomly inject errors into the batch building process to test our failure
    /// responses.
    #[error("nothing actually went wrong, failure was injected on purpose")]
    InjectedFailure,
}

// Store errors
// =================================================================================================

/// Errors returned by the [`StoreClient`](crate::store::StoreClient).
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("gRPC client error")]
    GrpcClientError(#[from] tonic::Status),
    #[error("malformed response from store: {0}")]
    MalformedResponse(String),
    #[error("failed to parse response")]
    DeserializationError(#[from] ConversionError),
}
