#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Note {
    #[prost(fixed32, tag = "1")]
    pub block_num: u32,
    #[prost(uint32, tag = "2")]
    pub note_index: u32,
    #[prost(message, optional, tag = "3")]
    pub note_id: ::core::option::Option<super::digest::Digest>,
    #[prost(message, optional, tag = "4")]
    pub sender: ::core::option::Option<super::account::AccountId>,
    #[prost(fixed64, tag = "5")]
    pub tag: u64,
    #[prost(message, optional, tag = "7")]
    pub merkle_path: ::core::option::Option<super::merkle::MerklePath>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NoteSyncRecord {
    #[prost(uint32, tag = "1")]
    pub note_index: u32,
    #[prost(message, optional, tag = "2")]
    pub note_id: ::core::option::Option<super::digest::Digest>,
    #[prost(message, optional, tag = "3")]
    pub sender: ::core::option::Option<super::account::AccountId>,
    #[prost(fixed64, tag = "4")]
    pub tag: u64,
    #[prost(message, optional, tag = "6")]
    pub merkle_path: ::core::option::Option<super::merkle::MerklePath>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NoteCreated {
    #[prost(uint32, tag = "1")]
    pub batch_index: u32,
    #[prost(uint32, tag = "2")]
    pub note_index: u32,
    #[prost(message, optional, tag = "3")]
    pub note_id: ::core::option::Option<super::digest::Digest>,
    #[prost(message, optional, tag = "4")]
    pub sender: ::core::option::Option<super::account::AccountId>,
    #[prost(fixed64, tag = "5")]
    pub tag: u64,
}
