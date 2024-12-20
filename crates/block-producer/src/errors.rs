use miden_node_proto::errors::ConversionError;
use miden_node_utils::formatting::format_opt;
use miden_objects::{
    accounts::AccountId,
    crypto::merkle::MerkleError,
    notes::{NoteId, Nullifier},
    transaction::TransactionId,
    AccountDeltaError, Digest, TransactionInputError,
};
use miden_processor::ExecutionError;
use thiserror::Error;
use tokio::task::JoinError;

use crate::mempool::BlockNumber;

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
    #[error("Input notes with given nullifiers were already consumed by another transaction")]
    InputNotesAlreadyConsumed(Vec<Nullifier>),

    /// Unauthenticated transaction notes were not found in the store or in outputs of in-flight
    /// transactions
    #[error(
        "Unauthenticated transaction notes were not found in the store or in outputs of in-flight transactions: {0:?}"
    )]
    UnauthenticatedNotesNotFound(Vec<NoteId>),

    #[error("Output note IDs already used: {0:?}")]
    OutputNotesAlreadyExist(Vec<NoteId>),

    /// The account's initial hash did not match the current account's hash
    #[error("Incorrect account's initial hash ({tx_initial_account_hash}, current: {})", format_opt(.current_account_hash.as_ref()))]
    IncorrectAccountInitialHash {
        tx_initial_account_hash: Digest,
        current_account_hash: Option<Digest>,
    },

    /// Failed to retrieve transaction inputs from the store
    ///
    /// TODO: Make this an "internal error". Q: Should we have a single `InternalError` enum for
    /// all internal errors that can occur across the system?
    #[error("Failed to retrieve transaction inputs from the store: {0}")]
    StoreConnectionFailed(#[from] StoreError),

    #[error("Transaction input error: {0}")]
    TransactionInputError(#[from] TransactionInputError),

    /// Failed to verify the transaction execution proof
    #[error("Invalid transaction proof error for transaction: {0}")]
    InvalidTransactionProof(TransactionId),
}

// Transaction adding errors
// =================================================================================================

#[derive(Debug, Error)]
pub enum AddTransactionError {
    #[error("Transaction verification failed: {0}")]
    VerificationFailed(#[from] VerifyTxError),

    #[error("Transaction input data is stale. Required data from {stale_limit} or newer, but inputs are from {input_block}.")]
    StaleInputs {
        input_block: BlockNumber,
        stale_limit: BlockNumber,
    },

    #[error("Deserialization failed: {0}")]
    DeserializationError(String),

    #[error("Transaction expired at {expired_at} and chain tip is {chain_tip}")]
    Expired {
        expired_at: BlockNumber,
        chain_tip: BlockNumber,
    },
}

impl From<AddTransactionError> for tonic::Status {
    fn from(value: AddTransactionError) -> Self {
        use AddTransactionError::*;
        match value {
            VerificationFailed(VerifyTxError::InputNotesAlreadyConsumed(_))
            | VerificationFailed(VerifyTxError::UnauthenticatedNotesNotFound(_))
            | VerificationFailed(VerifyTxError::OutputNotesAlreadyExist(_))
            | VerificationFailed(VerifyTxError::IncorrectAccountInitialHash { .. })
            | VerificationFailed(VerifyTxError::InvalidTransactionProof(_))
            | Expired { .. }
            | DeserializationError(_) => Self::invalid_argument(value.to_string()),

            // Internal errors which should not be communicated to the user.
            VerificationFailed(VerifyTxError::TransactionInputError(_))
            | VerificationFailed(VerifyTxError::StoreConnectionFailed(_))
            | StaleInputs { .. } => Self::internal("Internal error"),
        }
    }
}

// Batch building errors
// =================================================================================================

/// Error encountered while building a batch.
#[derive(Debug, Error)]
pub enum BuildBatchError {
    #[error("Duplicated unauthenticated transaction input note ID in the batch: {0}")]
    DuplicateUnauthenticatedNote(NoteId),

    #[error("Duplicated transaction output note ID in the batch: {0}")]
    DuplicateOutputNote(NoteId),

    #[error("Note hashes mismatch for note {id}: (input: {input_hash}, output: {output_hash})")]
    NoteHashesMismatch {
        id: NoteId,
        input_hash: Digest,
        output_hash: Digest,
    },

    #[error("Failed to merge transaction delta into account {account_id}: {error}")]
    AccountUpdateError {
        account_id: AccountId,
        error: AccountDeltaError,
    },

    #[error("Nothing actually went wrong, failure was injected on purpose")]
    InjectedFailure,

    #[error("Batch proving task panic'd")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Fetching inputs from store failed")]
    StoreError(#[from] StoreError),
}

// Block prover errors
// =================================================================================================

#[derive(Debug, Error)]
pub enum BlockProverError {
    #[error("Received invalid merkle path")]
    InvalidMerklePaths(MerkleError),
    #[error("Program execution failed")]
    ProgramExecutionFailed(ExecutionError),
    #[error("Failed to retrieve {0} root from stack outputs")]
    InvalidRootOutput(&'static str),
}

// Block building errors
// =================================================================================================

#[derive(Debug, Error)]
pub enum BuildBlockError {
    #[error("failed to compute new block: {0}")]
    BlockProverFailed(#[from] BlockProverError),
    #[error("failed to apply block: {0}")]
    ApplyBlockFailed(#[source] StoreError),
    #[error("failed to get block inputs from store: {0}")]
    GetBlockInputsFailed(#[source] StoreError),
    #[error("store did not produce data for account: {0}")]
    MissingAccountInput(AccountId),
    #[error("store produced extra account data. Offending accounts: {0:?}")]
    ExtraStoreData(Vec<AccountId>),
    #[error("no matching state transition found for account {0}. Current account state is {1}, remaining updates: {2:?}")]
    InconsistentAccountStateTransition(AccountId, Digest, Vec<Digest>),
    #[error("transaction batches and store don't produce the same nullifiers. Offending nullifiers: {0:?}")]
    InconsistentNullifiers(Vec<Nullifier>),
    #[error("unauthenticated transaction notes not found in the store or in outputs of other transactions in the block: {0:?}")]
    UnauthenticatedNotesNotFound(Vec<NoteId>),
    #[error("failed to merge transaction delta into account {account_id}: {error}")]
    AccountUpdateError {
        account_id: AccountId,
        error: AccountDeltaError,
    },
    #[error("nothing actually went wrong, failure was injected on purpose")]
    InjectedFailure,
}

// Store errors
// =================================================================================================

/// Errors returned by the [StoreClient](crate::store::StoreClient).
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("gRPC client error")]
    GrpcClientError(#[from] tonic::Status),
    #[error("malformed response from store: {0}")]
    MalformedResponse(String),
    #[error("failed to parse response")]
    DeserializationError(#[from] ConversionError),
}
