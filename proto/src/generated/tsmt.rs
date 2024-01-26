#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NullifierLeaf {
    #[prost(message, optional, tag = "1")]
    pub key: ::core::option::Option<super::digest::Digest>,
    #[prost(uint32, tag = "2")]
    pub block_num: u32,
}
/// A Nullifier proof is a special case of a TSMT proof, where the leaf is a u32.
///
/// This proof supports both inclusion and non-inclusion proofs. This is an inclusion proof if target
/// key is in the `leaves` list, non-inclusion otherwise.
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NullifierProof {
    /// For depth 64 this may have multiple entries. The list is empty if there is no leaf. If the
    /// list is non empty, a check for the target value has to be done to determine if it is a
    /// inclusion or non-inclusion proof.
    #[prost(message, repeated, tag = "1")]
    pub leaves: ::prost::alloc::vec::Vec<NullifierLeaf>,
    /// The merkle path authenticating the leaf values.
    #[prost(message, repeated, tag = "2")]
    pub merkle_path: ::prost::alloc::vec::Vec<super::digest::Digest>,
}
