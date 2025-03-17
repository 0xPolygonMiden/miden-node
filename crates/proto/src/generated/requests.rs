// This file is @generated by prost-build.
/// Applies changes of a new block to the DB and in-memory data structures.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ApplyBlockRequest {
    /// Block data encoded using \[winter_utils::Serializable\] implementation for
    /// \[miden_objects::block::Block\].
    #[prost(bytes = "vec", tag = "1")]
    pub block: ::prost::alloc::vec::Vec<u8>,
}
/// Returns a list of nullifiers that match the specified prefixes and are recorded in the node.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CheckNullifiersByPrefixRequest {
    /// Number of bits used for nullifier prefix. Currently the only supported value is 16.
    #[prost(uint32, tag = "1")]
    pub prefix_len: u32,
    /// List of nullifiers to check. Each nullifier is specified by its prefix with length equal
    /// to `prefix_len`.
    #[prost(uint32, repeated, tag = "2")]
    pub nullifiers: ::prost::alloc::vec::Vec<u32>,
    /// Block number from which the nullifiers are requested (inclusive).
    #[prost(fixed32, tag = "3")]
    pub block_num: u32,
}
/// Returns a nullifier proof for each of the requested nullifiers.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CheckNullifiersRequest {
    /// List of nullifiers to return proofs for.
    #[prost(message, repeated, tag = "1")]
    pub nullifiers: ::prost::alloc::vec::Vec<super::digest::Digest>,
}
/// Returns the block header corresponding to the requested block number, as well as the merkle
/// path and current forest which validate the block's inclusion in the chain.
///
/// The Merkle path is an MMR proof for the block's leaf, based on the current chain length.
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct GetBlockHeaderByNumberRequest {
    /// The target block height, defaults to latest if not provided.
    #[prost(uint32, optional, tag = "1")]
    pub block_num: ::core::option::Option<u32>,
    /// Whether or not to return authentication data for the block header.
    #[prost(bool, optional, tag = "2")]
    pub include_mmr_proof: ::core::option::Option<bool>,
}
/// State synchronization request.
///
/// Specifies state updates the client is interested in. The server will return the first block which
/// contains a note matching `note_tags` or the chain tip. And the corresponding updates to
/// `account_ids` for that block range.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncStateRequest {
    /// Last block known by the client. The response will contain data starting from the next block,
    /// until the first block which contains a note of matching the requested tag, or the chain tip
    /// if there are no notes.
    #[prost(fixed32, tag = "1")]
    pub block_num: u32,
    /// Accounts' hash to include in the response.
    ///
    /// An account hash will be included if-and-only-if it is the latest update. Meaning it is
    /// possible there was an update to the account for the given range, but if it is not the latest,
    /// it won't be included in the response.
    #[prost(message, repeated, tag = "2")]
    pub account_ids: ::prost::alloc::vec::Vec<super::account::AccountId>,
    /// Specifies the tags which the client is interested in.
    #[prost(fixed32, repeated, tag = "3")]
    pub note_tags: ::prost::alloc::vec::Vec<u32>,
}
/// Note synchronization request.
///
/// Specifies note tags that client is interested in. The server will return the first block which
/// contains a note matching `note_tags` or the chain tip.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncNoteRequest {
    /// Last block known by the client. The response will contain data starting from the next block,
    /// until the first block which contains a note of matching the requested tag.
    #[prost(fixed32, tag = "1")]
    pub block_num: u32,
    /// Specifies the tags which the client is interested in.
    #[prost(fixed32, repeated, tag = "2")]
    pub note_tags: ::prost::alloc::vec::Vec<u32>,
}
/// Returns data required to prove the next block.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBlockInputsRequest {
    /// IDs of all accounts updated in the proposed block for which to retrieve account witnesses.
    #[prost(message, repeated, tag = "1")]
    pub account_ids: ::prost::alloc::vec::Vec<super::account::AccountId>,
    /// Nullifiers of all notes consumed by the block for which to retrieve witnesses.
    ///
    /// Due to note erasure it will generally not be possible to know the exact set of nullifiers
    /// a block will create, unless we pre-execute note erasure. So in practice, this set of
    /// nullifiers will be the set of nullifiers of all proven batches in the block, which is a
    /// superset of the nullifiers the block may create.
    ///
    /// However, if it is known that a certain note will be erased, it would not be necessary to
    /// provide a nullifier witness for it.
    #[prost(message, repeated, tag = "2")]
    pub nullifiers: ::prost::alloc::vec::Vec<super::digest::Digest>,
    /// Array of note IDs for which to retrieve note inclusion proofs, **if they exist in the store**.
    #[prost(message, repeated, tag = "3")]
    pub unauthenticated_notes: ::prost::alloc::vec::Vec<super::digest::Digest>,
    /// Array of block numbers referenced by all batches in the block.
    #[prost(fixed32, repeated, tag = "4")]
    pub reference_blocks: ::prost::alloc::vec::Vec<u32>,
}
/// Returns the inputs for a transaction batch.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBatchInputsRequest {
    /// List of unauthenticated notes to be queried from the database.
    #[prost(message, repeated, tag = "1")]
    pub note_ids: ::prost::alloc::vec::Vec<super::digest::Digest>,
    /// Set of block numbers referenced by transactions.
    #[prost(fixed32, repeated, tag = "2")]
    pub reference_blocks: ::prost::alloc::vec::Vec<u32>,
}
/// Returns data required to validate a new transaction.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetTransactionInputsRequest {
    /// ID of the account against which a transaction is executed.
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::account::AccountId>,
    /// Set of nullifiers consumed by this transaction.
    #[prost(message, repeated, tag = "2")]
    pub nullifiers: ::prost::alloc::vec::Vec<super::digest::Digest>,
    /// Set of unauthenticated notes to check for existence on-chain.
    ///
    /// These are notes which were not on-chain at the state the transaction was proven,
    /// but could by now be present.
    #[prost(message, repeated, tag = "3")]
    pub unauthenticated_notes: ::prost::alloc::vec::Vec<super::digest::Digest>,
}
/// Submits proven transaction to the Miden network.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitProvenTransactionRequest {
    /// Transaction encoded using \[winter_utils::Serializable\] implementation for
    /// \[miden_objects::transaction::proven_tx::ProvenTransaction\].
    #[prost(bytes = "vec", tag = "1")]
    pub transaction: ::prost::alloc::vec::Vec<u8>,
}
/// Returns a list of notes matching the provided note IDs.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetNotesByIdRequest {
    /// List of notes to be queried from the database.
    #[prost(message, repeated, tag = "1")]
    pub note_ids: ::prost::alloc::vec::Vec<super::digest::Digest>,
}
/// Returns the latest state of an account with the specified ID.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAccountDetailsRequest {
    /// Account ID to get details.
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::account::AccountId>,
}
/// Retrieves block data by given block number.
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct GetBlockByNumberRequest {
    /// The block number of the target block.
    #[prost(fixed32, tag = "1")]
    pub block_num: u32,
}
/// Returns delta of the account states in the range from `from_block_num` (exclusive) to
/// `to_block_num` (inclusive).
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAccountStateDeltaRequest {
    /// ID of the account for which the delta is requested.
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::account::AccountId>,
    /// Block number from which the delta is requested (exclusive).
    #[prost(fixed32, tag = "2")]
    pub from_block_num: u32,
    /// Block number up to which the delta is requested (inclusive).
    #[prost(fixed32, tag = "3")]
    pub to_block_num: u32,
}
/// Returns the latest state proofs of the specified accounts.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAccountProofsRequest {
    /// A list of account requests, including map keys + values.
    #[prost(message, repeated, tag = "1")]
    pub account_requests: ::prost::alloc::vec::Vec<
        get_account_proofs_request::AccountRequest,
    >,
    /// Optional flag to include account headers and account code in the response. If false, storage
    /// requests are also ignored. False by default.
    #[prost(bool, optional, tag = "2")]
    pub include_headers: ::core::option::Option<bool>,
    /// Account code commitments corresponding to the last-known `AccountCode` for requested
    /// accounts. Responses will include only the ones that are not known to the caller.
    /// These are not associated with a specific account but rather, they will be matched against
    /// all requested accounts.
    #[prost(message, repeated, tag = "3")]
    pub code_commitments: ::prost::alloc::vec::Vec<super::digest::Digest>,
}
/// Nested message and enum types in `GetAccountProofsRequest`.
pub mod get_account_proofs_request {
    /// Represents per-account requests where each account ID has its own list of
    /// (storage_slot_index, map_keys) pairs.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct AccountRequest {
        /// The account ID for this request.
        #[prost(message, optional, tag = "1")]
        pub account_id: ::core::option::Option<super::super::account::AccountId>,
        /// List of storage requests for this account.
        #[prost(message, repeated, tag = "2")]
        pub storage_requests: ::prost::alloc::vec::Vec<StorageRequest>,
    }
    /// Represents a storage slot index and the associated map keys.
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct StorageRequest {
        /// Storage slot index (\[0..255\])
        #[prost(uint32, tag = "1")]
        pub storage_slot_index: u32,
        /// A list of map keys (Digests) associated with this storage slot.
        #[prost(message, repeated, tag = "2")]
        pub map_keys: ::prost::alloc::vec::Vec<super::super::digest::Digest>,
    }
}
/// Returns a list of unconsumed network notes using pagination.
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct GetUnconsumedNetworkNotesRequest {
    /// Page number to retrieve.
    #[prost(uint64, tag = "1")]
    pub page: u64,
    /// Number of notes to retrieve per page.
    #[prost(uint64, tag = "2")]
    pub limit: u64,
}
/// Creates a new network transaction.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CreateNetworkTransactionRequest {
    /// The network note that creates the transaction.
    #[prost(message, optional, tag = "1")]
    pub note: ::core::option::Option<super::note::Note>,
    /// Id of the transaction that created the note.
    #[prost(message, optional, tag = "2")]
    pub transaction_id: ::core::option::Option<super::digest::Digest>,
}
/// Updates the status of a network transaction.
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct UpdateNetworkTransactionRequest {
    /// Id of the transaction to update.
    #[prost(message, optional, tag = "1")]
    pub transaction_id: ::core::option::Option<super::digest::Digest>,
    /// New status of the transaction.
    #[prost(enumeration = "super::transaction::NetworkTransactionStatus", tag = "2")]
    pub status: i32,
}
