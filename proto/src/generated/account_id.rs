#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountId {
    /// A miden account is defined with a little bit of proof-of-work, the id itself
    /// is defined as the first word of a hash digest. For this reason account ids
    /// can be considered as random values, because of that the encoding bellow uses
    /// fixed 64 bits, instead of zig-zag encoding.
    #[prost(fixed64, tag = "1")]
    pub id: u64,
}
