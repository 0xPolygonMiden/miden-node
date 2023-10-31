#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CheckNullifiersResponse {
    /// Each requested nullifier has its corresponding nullifier proof at the
    /// same position.
    #[prost(message, repeated, tag = "1")]
    pub proofs: ::prost::alloc::vec::Vec<super::tsmt::NullifierProof>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FetchBlockHeaderByNumberResponse {
    #[prost(message, optional, tag = "1")]
    pub block_header: ::core::option::Option<super::block_header::BlockHeader>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountHashUpdate {
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<super::account_id::AccountId>,
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
    #[prost(uint32, tag = "2")]
    pub block_num: u32,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NoteSyncRecord {
    #[prost(uint32, tag = "2")]
    pub note_index: u32,
    #[prost(message, optional, tag = "3")]
    pub note_hash: ::core::option::Option<super::digest::Digest>,
    #[prost(uint64, tag = "4")]
    pub sender: u64,
    #[prost(uint64, tag = "5")]
    pub tag: u64,
    #[prost(uint32, tag = "6")]
    pub num_assets: u32,
    #[prost(message, optional, tag = "7")]
    pub merkle_path: ::core::option::Option<super::merkle::MerklePath>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SyncStateResponse {
    /// number of the latest block in the chain
    #[prost(uint32, tag = "1")]
    pub chain_tip: u32,
    /// block header of the block with the first note matching the specified criteria
    #[prost(message, optional, tag = "2")]
    pub block_header: ::core::option::Option<super::block_header::BlockHeader>,
    /// data needed to update the partial MMR from `block_ref` to `block_header.block_num`
    #[prost(message, optional, tag = "3")]
    pub mmr_delta: ::core::option::Option<super::mmr::MmrDelta>,
    /// Merkle path in the updated chain MMR to the block at `block_header.block_num`
    #[prost(message, optional, tag = "4")]
    pub block_path: ::core::option::Option<super::merkle::MerklePath>,
    /// a list of account hashes updated after `block_ref` but not after `block_header.block_num`
    #[prost(message, repeated, tag = "5")]
    pub accounts: ::prost::alloc::vec::Vec<AccountHashUpdate>,
    /// a list of all notes together with the Merkle paths from `block_header.note_root`
    #[prost(message, repeated, tag = "6")]
    pub notes: ::prost::alloc::vec::Vec<NoteSyncRecord>,
    /// a list of nullifiers created between `block_ref` and `block_header.block_num`
    #[prost(message, repeated, tag = "7")]
    pub nullifiers: ::prost::alloc::vec::Vec<NullifierUpdate>,
}
