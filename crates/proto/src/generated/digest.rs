// This file is @generated by prost-build.
/// A hash digest, the result of a hash function.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
#[prost(skip_debug)]
pub struct Digest {
    #[prost(fixed64, tag = "1")]
    pub d0: u64,
    #[prost(fixed64, tag = "2")]
    pub d1: u64,
    #[prost(fixed64, tag = "3")]
    pub d2: u64,
    #[prost(fixed64, tag = "4")]
    pub d3: u64,
}
