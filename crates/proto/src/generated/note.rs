// This file is @generated by prost-build.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NoteMetadata {
    #[prost(message, optional, tag = "1")]
    pub sender: ::core::option::Option<super::account::AccountId>,
    #[prost(enumeration = "NoteType", tag = "2")]
    pub note_type: i32,
    #[prost(fixed32, tag = "3")]
    pub tag: u32,
    #[prost(fixed64, tag = "4")]
    pub aux: u64,
}
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
    pub metadata: ::core::option::Option<NoteMetadata>,
    #[prost(message, optional, tag = "5")]
    pub merkle_path: ::core::option::Option<super::merkle::MerklePath>,
    /// This field will be present when the note is on-chain.
    /// details contain the `Note` in a serialized format.
    #[prost(bytes = "vec", optional, tag = "6")]
    pub details: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NoteSyncRecord {
    #[prost(uint32, tag = "1")]
    pub note_index: u32,
    #[prost(message, optional, tag = "2")]
    pub note_id: ::core::option::Option<super::digest::Digest>,
    #[prost(message, optional, tag = "3")]
    pub metadata: ::core::option::Option<NoteMetadata>,
    #[prost(message, optional, tag = "4")]
    pub merkle_path: ::core::option::Option<super::merkle::MerklePath>,
}
/// These values should always match the values in
/// <https://github.com/0xPolygonMiden/miden-base/blob/next/objects/src/notes/note_type.rs#L10-L12>
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum NoteType {
    /// PHANTOM variant exists so that the number representations map correctly
    Phantom = 0,
    Public = 1,
    OffChain = 2,
    Encrypted = 3,
}
impl NoteType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            NoteType::Phantom => "PHANTOM",
            NoteType::Public => "PUBLIC",
            NoteType::OffChain => "OFF_CHAIN",
            NoteType::Encrypted => "ENCRYPTED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "PHANTOM" => Some(Self::Phantom),
            "PUBLIC" => Some(Self::Public),
            "OFF_CHAIN" => Some(Self::OffChain),
            "ENCRYPTED" => Some(Self::Encrypted),
            _ => None,
        }
    }
}
