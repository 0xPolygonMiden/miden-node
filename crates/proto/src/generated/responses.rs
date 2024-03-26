#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ApplyBlockResponse {}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CheckNullifiersResponse {
    /// Each requested nullifier has its corresponding nullifier proof at the same position.
    #[prost(message, repeated, tag = "1")]
    pub proofs: ::prost::alloc::vec::Vec<super::smt::SmtOpening>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBlockHeaderByNumberResponse {
    #[prost(message, optional, tag = "1")]
    pub block_header: ::core::option::Option<super::block_header::BlockHeader>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountHashUpdate {
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::account::AccountId>,
    #[prost(message, optional, tag = "2")]
    pub account_hash: ::core::option::Option<super::digest::Digest>,
    #[prost(uint32, tag = "3")]
    pub block_num: u32,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NullifierUpdate {
    #[prost(message, optional, tag = "1")]
    pub nullifier: ::core::option::Option<super::digest::Digest>,
    #[prost(fixed32, tag = "2")]
    pub block_num: u32,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncStateResponse {
    /// number of the latest block in the chain
    #[prost(fixed32, tag = "1")]
    pub chain_tip: u32,
    /// block header of the block with the first note matching the specified criteria
    #[prost(message, optional, tag = "2")]
    pub block_header: ::core::option::Option<super::block_header::BlockHeader>,
    /// data needed to update the partial MMR from `block_num + 1` to `block_header.block_num`
    #[prost(message, optional, tag = "3")]
    pub mmr_delta: ::core::option::Option<super::mmr::MmrDelta>,
    /// a list of account hashes updated after `block_num + 1` but not after `block_header.block_num`
    #[prost(message, repeated, tag = "5")]
    pub accounts: ::prost::alloc::vec::Vec<AccountHashUpdate>,
    /// a list of all notes together with the Merkle paths from `block_header.note_root`
    #[prost(message, repeated, tag = "6")]
    pub notes: ::prost::alloc::vec::Vec<super::note::NoteSyncRecord>,
    /// a list of nullifiers created between `block_num + 1` and `block_header.block_num`
    #[prost(message, repeated, tag = "7")]
    pub nullifiers: ::prost::alloc::vec::Vec<NullifierUpdate>,
}
/// An account returned as a response to the GetBlockInputs
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountBlockInputRecord {
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::account::AccountId>,
    #[prost(message, optional, tag = "2")]
    pub account_hash: ::core::option::Option<super::digest::Digest>,
    #[prost(message, optional, tag = "3")]
    pub proof: ::core::option::Option<super::merkle::MerklePath>,
}
/// A nullifier returned as a response to the GetBlockInputs
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NullifierBlockInputRecord {
    #[prost(message, optional, tag = "1")]
    pub nullifier: ::core::option::Option<super::digest::Digest>,
    #[prost(message, optional, tag = "2")]
    pub opening: ::core::option::Option<super::smt::SmtOpening>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBlockInputsResponse {
    /// The latest block header
    #[prost(message, optional, tag = "1")]
    pub block_header: ::core::option::Option<super::block_header::BlockHeader>,
    /// Peaks of the above block's mmr, The `forest` value is equal to the block number.
    #[prost(message, repeated, tag = "2")]
    pub mmr_peaks: ::prost::alloc::vec::Vec<super::digest::Digest>,
    /// The hashes of the requested accouts and their authentication paths
    #[prost(message, repeated, tag = "3")]
    pub account_states: ::prost::alloc::vec::Vec<AccountBlockInputRecord>,
    /// The requested nullifiers and their authentication paths
    #[prost(message, repeated, tag = "4")]
    pub nullifiers: ::prost::alloc::vec::Vec<NullifierBlockInputRecord>,
}
/// An account returned as a response to the GetTransactionInputs
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountTransactionInputRecord {
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::account::AccountId>,
    /// The latest account hash, zero hash if the account doesn't exist.
    #[prost(message, optional, tag = "2")]
    pub account_hash: ::core::option::Option<super::digest::Digest>,
}
/// A nullifier returned as a response to the GetTransactionInputs
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NullifierTransactionInputRecord {
    #[prost(message, optional, tag = "1")]
    pub nullifier: ::core::option::Option<super::digest::Digest>,
    /// The block at which the nullifier has been consumed, zero if not consumed.
    #[prost(fixed32, tag = "2")]
    pub block_num: u32,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetTransactionInputsResponse {
    #[prost(message, optional, tag = "1")]
    pub account_state: ::core::option::Option<AccountTransactionInputRecord>,
    #[prost(message, repeated, tag = "2")]
    pub nullifiers: ::prost::alloc::vec::Vec<NullifierTransactionInputRecord>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitProvenTransactionResponse {}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListNullifiersResponse {
    /// Lists all nullifiers of the current chain
    #[prost(message, repeated, tag = "1")]
    pub nullifiers: ::prost::alloc::vec::Vec<super::smt::SmtLeafEntry>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListAccountsResponse {
    /// Lists all accounts of the current chain
    #[prost(message, repeated, tag = "1")]
    pub accounts: ::prost::alloc::vec::Vec<super::account::AccountInfo>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListNotesResponse {
    /// Lists all notes of the current chain
    #[prost(message, repeated, tag = "1")]
    pub notes: ::prost::alloc::vec::Vec<super::note::Note>,
}
