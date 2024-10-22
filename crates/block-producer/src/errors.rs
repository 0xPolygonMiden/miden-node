use miden_node_proto::errors::ConversionError;
use miden_node_utils::formatting::format_opt;
use miden_objects::{
    accounts::AccountId,
    crypto::merkle::{MerkleError, MmrError},
    notes::{NoteId, Nullifier},
    transaction::{ProvenTransaction, TransactionId},
    AccountDeltaError, Digest, TransactionInputError, MAX_ACCOUNTS_PER_BATCH,
    MAX_BATCHES_PER_BLOCK, MAX_INPUT_NOTES_PER_BATCH, MAX_OUTPUT_NOTES_PER_BATCH,
};
use miden_processor::ExecutionError;
use thiserror::Error;

use crate::mempool::BlockNumber;

// Transaction verification errors
// =================================================================================================

#[derive(Debug, PartialEq, Eq, Error)]
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
    StoreConnectionFailed(#[from] TxInputsError),

    #[error("Transaction input error: {0}")]
    TransactionInputError(#[from] TransactionInputError),

    /// Failed to verify the transaction execution proof
    #[error("Invalid transaction proof error for transaction: {0}")]
    InvalidTransactionProof(TransactionId),
}

// Transaction adding errors
// =================================================================================================

#[derive(Debug, PartialEq, Eq, Error)]
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

/// Error that may happen while building a transaction batch.
///
/// These errors are returned from the batch builder to the transaction queue, instead of
/// dropping the transactions, they are included into the error values, so that the transaction
/// queue can re-queue them.
#[derive(Debug, PartialEq, Eq, Error)]
pub enum BuildBatchError {
    #[error(
        "Too many input notes in the batch. Got: {0}, limit: {}",
        MAX_INPUT_NOTES_PER_BATCH
    )]
    TooManyInputNotes(usize, Vec<ProvenTransaction>),

    #[error(
        "Too many notes created in the batch. Got: {0}, limit: {}",
        MAX_OUTPUT_NOTES_PER_BATCH
    )]
    TooManyNotesCreated(usize, Vec<ProvenTransaction>),

    #[error(
        "Too many account updates in the batch. Got: {}, limit: {}",
        .0.len(),
        MAX_ACCOUNTS_PER_BATCH
    )]
    TooManyAccountsInBatch(Vec<ProvenTransaction>),

    #[error("Failed to create notes SMT: {0}")]
    NotesSmtError(MerkleError, Vec<ProvenTransaction>),

    #[error("Failed to get note paths: {0}")]
    NotePathsError(NotePathsError, Vec<ProvenTransaction>),

    #[error("Duplicated unauthenticated transaction input note ID in the batch: {0}")]
    DuplicateUnauthenticatedNote(NoteId, Vec<ProvenTransaction>),

    #[error("Duplicated transaction output note ID in the batch: {0}")]
    DuplicateOutputNote(NoteId, Vec<ProvenTransaction>),

    #[error("Unauthenticated transaction notes not found in the store: {0:?}")]
    UnauthenticatedNotesNotFound(Vec<NoteId>, Vec<ProvenTransaction>),

    #[error("Note hashes mismatch for note {id}: (input: {input_hash}, output: {output_hash})")]
    NoteHashesMismatch {
        id: NoteId,
        input_hash: Digest,
        output_hash: Digest,
        txs: Vec<ProvenTransaction>,
    },

    #[error("Failed to merge transaction delta into account {account_id}: {error}")]
    AccountUpdateError {
        account_id: AccountId,
        error: AccountDeltaError,
        txs: Vec<ProvenTransaction>,
    },
}

impl BuildBatchError {
    pub fn into_transactions(self) -> Vec<ProvenTransaction> {
        match self {
            BuildBatchError::TooManyInputNotes(_, txs) => txs,
            BuildBatchError::TooManyNotesCreated(_, txs) => txs,
            BuildBatchError::TooManyAccountsInBatch(txs) => txs,
            BuildBatchError::NotesSmtError(_, txs) => txs,
            BuildBatchError::NotePathsError(_, txs) => txs,
            BuildBatchError::DuplicateUnauthenticatedNote(_, txs) => txs,
            BuildBatchError::DuplicateOutputNote(_, txs) => txs,
            BuildBatchError::UnauthenticatedNotesNotFound(_, txs) => txs,
            BuildBatchError::NoteHashesMismatch { txs, .. } => txs,
            BuildBatchError::AccountUpdateError { txs, .. } => txs,
        }
    }
}

// Block prover errors
// =================================================================================================

#[derive(Debug, PartialEq, Eq, Error)]
pub enum BlockProverError {
    #[error("Received invalid merkle path")]
    InvalidMerklePaths(MerkleError),
    #[error("Program execution failed")]
    ProgramExecutionFailed(ExecutionError),
    #[error("Failed to retrieve {0} root from stack outputs")]
    InvalidRootOutput(&'static str),
}

// Block inputs errors
// =================================================================================================

#[allow(clippy::enum_variant_names)]
#[derive(Debug, PartialEq, Eq, Error)]
pub enum BlockInputsError {
    #[error("failed to parse protobuf message: {0}")]
    ConversionError(#[from] ConversionError),
    #[error("MmrPeaks error: {0}")]
    MmrPeaksError(#[from] MmrError),
    #[error("gRPC client failed with error: {0}")]
    GrpcClientError(String),
}

// Note paths errors
// =================================================================================================

#[allow(clippy::enum_variant_names)]
#[derive(Debug, PartialEq, Eq, Error)]
pub enum NotePathsError {
    #[error("failed to parse protobuf message: {0}")]
    ConversionError(#[from] ConversionError),
    #[error("gRPC client failed with error: {0}")]
    GrpcClientError(String),
}

// Block applying errors
// =================================================================================================

#[derive(Debug, PartialEq, Eq, Error)]
pub enum ApplyBlockError {
    #[error("gRPC client failed with error: {0}")]
    GrpcClientError(String),
}

// Block building errors
// =================================================================================================

#[derive(Debug, PartialEq, Eq, Error)]
pub enum BuildBlockError {
    #[error("failed to compute new block: {0}")]
    BlockProverFailed(#[from] BlockProverError),
    #[error("failed to apply block: {0}")]
    ApplyBlockFailed(#[from] ApplyBlockError),
    #[error("failed to get block inputs from store: {0}")]
    GetBlockInputsFailed(#[from] BlockInputsError),
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
    #[error("too many batches in block. Got: {0}, max: {MAX_BATCHES_PER_BLOCK}")]
    TooManyBatchesInBlock(usize),
    #[error("Failed to merge transaction delta into account {account_id}: {error}")]
    AccountUpdateError {
        account_id: AccountId,
        error: AccountDeltaError,
    },
}

// Transaction inputs errors
// =================================================================================================

#[derive(Debug, PartialEq, Eq, Error)]
pub enum TxInputsError {
    #[error("gRPC client failed with error: {0}")]
    GrpcClientError(String),
    #[error("malformed response from store: {0}")]
    MalformedResponse(String),
    #[error("failed to parse protobuf message: {0}")]
    ConversionError(#[from] ConversionError),
    #[error("dummy")]
    Dummy,
}
