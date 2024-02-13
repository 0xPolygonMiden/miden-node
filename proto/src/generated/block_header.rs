#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BlockHeader {
    /// the hash of the previous blocks header.
    #[prost(message, optional, tag = "1")]
    pub prev_hash: ::core::option::Option<super::digest::Digest>,
    /// a unique sequential number of the current block.
    #[prost(fixed32, tag = "2")]
    pub block_num: u32,
    /// a commitment to an MMR of the entire chain where each block is a leaf.
    #[prost(message, optional, tag = "3")]
    pub chain_root: ::core::option::Option<super::digest::Digest>,
    /// a commitment to account database.
    #[prost(message, optional, tag = "4")]
    pub account_root: ::core::option::Option<super::digest::Digest>,
    /// a commitment to the nullifier database.
    #[prost(message, optional, tag = "5")]
    pub nullifier_root: ::core::option::Option<super::digest::Digest>,
    /// a commitment to all notes created in the current block.
    #[prost(message, optional, tag = "6")]
    pub note_root: ::core::option::Option<super::digest::Digest>,
    /// a commitment to a set of transaction batches executed as a part of this block.
    #[prost(message, optional, tag = "7")]
    pub batch_root: ::core::option::Option<super::digest::Digest>,
    /// a hash of a STARK proof attesting to the correct state transition.
    #[prost(message, optional, tag = "8")]
    pub proof_hash: ::core::option::Option<super::digest::Digest>,
    /// specifies the version of the protocol.
    #[prost(uint32, tag = "9")]
    pub version: u32,
    /// the time when the block was created.
    #[prost(fixed32, tag = "10")]
    pub timestamp: u32,
}
