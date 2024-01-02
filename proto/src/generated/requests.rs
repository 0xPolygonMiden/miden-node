#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EmptyRequest {}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountUpdate {
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::account::AccountId>,
    #[prost(message, optional, tag = "2")]
    pub account_hash: ::core::option::Option<super::digest::Digest>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ApplyBlockRequest {
    #[prost(message, optional, tag = "1")]
    pub block: ::core::option::Option<super::block_header::BlockHeader>,
    #[prost(message, repeated, tag = "2")]
    pub accounts: ::prost::alloc::vec::Vec<AccountUpdate>,
    #[prost(message, repeated, tag = "3")]
    pub nullifiers: ::prost::alloc::vec::Vec<super::digest::Digest>,
    #[prost(message, repeated, tag = "4")]
    pub notes: ::prost::alloc::vec::Vec<super::note::NoteCreated>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CheckNullifiersRequest {
    #[prost(message, repeated, tag = "1")]
    pub nullifiers: ::prost::alloc::vec::Vec<super::digest::Digest>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBlockHeaderByNumberRequest {
    /// The block number of the target block.
    ///
    /// If not provided, means latest know block.
    #[prost(uint32, optional, tag = "1")]
    pub block_num: ::core::option::Option<u32>,
}
/// State synchronization request.
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncStateRequest {
    /// Send updates to the client starting at this block.
    #[prost(uint32, tag = "1")]
    pub block_num: u32,
    #[prost(message, repeated, tag = "2")]
    pub account_ids: ::prost::alloc::vec::Vec<super::account::AccountId>,
    /// Tags and nullifiers are filters, both filters correspond to the high
    /// 16bits of the real values shifted to the right `>> 48`.
    #[prost(uint32, repeated, tag = "3")]
    pub note_tags: ::prost::alloc::vec::Vec<u32>,
    #[prost(uint32, repeated, tag = "4")]
    pub nullifiers: ::prost::alloc::vec::Vec<u32>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBlockInputsRequest {
    #[prost(message, repeated, tag = "1")]
    pub account_ids: ::prost::alloc::vec::Vec<super::account::AccountId>,
    #[prost(message, repeated, tag = "2")]
    pub nullifiers: ::prost::alloc::vec::Vec<super::digest::Digest>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetTransactionInputsRequest {
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::account::AccountId>,
    #[prost(message, repeated, tag = "2")]
    pub nullifiers: ::prost::alloc::vec::Vec<super::digest::Digest>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitProvenTransactionRequest {
    /// Transaction encoded using miden's native format
    #[prost(bytes = "vec", tag = "1")]
    pub transaction: ::prost::alloc::vec::Vec<u8>,
}
