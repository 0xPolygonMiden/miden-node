/// TODO: remove this message as it can be replaced by an internal domain type in the Store
/// component
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Note {
    #[prost(uint32, tag = "1")]
    pub block_num: u32,
    #[prost(uint32, tag = "2")]
    pub note_index: u32,
    #[prost(message, optional, tag = "3")]
    pub note_hash: ::core::option::Option<super::digest::Digest>,
    #[prost(fixed64, tag = "4")]
    pub sender: u64,
    #[prost(uint64, tag = "5")]
    pub tag: u64,
    #[prost(uint32, tag = "6")]
    pub num_assets: u32,
    #[prost(message, optional, tag = "7")]
    pub merkle_path: ::core::option::Option<super::merkle::MerklePath>,
}
/// TODO: change `sender` to AccountId
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NoteSyncRecord {
    #[prost(uint32, tag = "1")]
    pub note_index: u32,
    #[prost(message, optional, tag = "2")]
    pub note_hash: ::core::option::Option<super::digest::Digest>,
    #[prost(fixed64, tag = "3")]
    pub sender: u64,
    #[prost(uint64, tag = "4")]
    pub tag: u64,
    #[prost(uint32, tag = "5")]
    pub num_assets: u32,
    #[prost(message, optional, tag = "6")]
    pub merkle_path: ::core::option::Option<super::merkle::MerklePath>,
}
/// TODO: change `sender` to AccountId
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NoteCreated {
    #[prost(uint32, tag = "1")]
    pub note_index: u32,
    #[prost(message, optional, tag = "2")]
    pub note_hash: ::core::option::Option<super::digest::Digest>,
    #[prost(fixed64, tag = "3")]
    pub sender: u64,
    #[prost(uint64, tag = "4")]
    pub tag: u64,
    #[prost(uint32, tag = "5")]
    pub num_assets: u32,
}
