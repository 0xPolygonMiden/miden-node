#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
#[prost(skip_debug)]
pub struct AccountId {
    /// A miden account is defined with a little bit of proof-of-work, the id itself is defined as
    /// the first word of a hash digest. For this reason account ids can be considered as random
    /// values, because of that the encoding bellow uses fixed 64 bits, instead of zig-zag encoding.
    #[prost(fixed64, tag = "1")]
    pub id: u64,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountInfo {
    #[prost(message, optional, tag = "1")]
    pub account_id: ::core::option::Option<AccountId>,
    #[prost(message, optional, tag = "2")]
    pub account_hash: ::core::option::Option<super::digest::Digest>,
    #[prost(fixed32, tag = "3")]
    pub block_num: u32,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FungibleAsset {
    /// Faucet ID.
    #[prost(message, optional, tag = "1")]
    pub faucet_id: ::core::option::Option<AccountId>,
    /// Amount of asset.
    #[prost(uint64, tag = "2")]
    pub amount: u64,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NonFungibleAsset {
    /// Non-fungible asset in internal (`Word`) representation.
    #[prost(message, optional, tag = "1")]
    pub asset: ::core::option::Option<super::digest::Digest>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Asset {
    /// Asset enumeration.
    #[prost(oneof = "asset::Asset", tags = "1, 2")]
    pub asset: ::core::option::Option<asset::Asset>,
}
/// Nested message and enum types in `Asset`.
pub mod asset {
    /// Asset enumeration.
    #[derive(Eq, PartialOrd, Ord, Hash)]
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Asset {
        #[prost(message, tag = "1")]
        Fungible(super::FungibleAsset),
        #[prost(message, tag = "2")]
        NonFungible(super::NonFungibleAsset),
    }
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetVault {
    /// Assets vector.
    #[prost(message, repeated, tag = "1")]
    pub assets: ::prost::alloc::vec::Vec<Asset>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountStorage {
    /// Full account storage serialized using Miden serialization procedure.
    #[prost(bytes = "vec", tag = "1")]
    pub data: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountCode {
    /// Module AST tree serialized using Miden serialization procedure.
    #[prost(bytes = "vec", tag = "1")]
    pub module: ::prost::alloc::vec::Vec<u8>,
    /// Procedures vector.
    #[prost(message, repeated, tag = "2")]
    pub procedures: ::prost::alloc::vec::Vec<super::digest::Digest>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountFullDetails {
    /// Account ID.
    #[prost(message, optional, tag = "1")]
    pub id: ::core::option::Option<AccountId>,
    /// All account's assets.
    #[prost(message, optional, tag = "2")]
    pub vault: ::core::option::Option<AssetVault>,
    /// Account storage.
    #[prost(message, optional, tag = "3")]
    pub storage: ::core::option::Option<AccountStorage>,
    /// Account code.
    #[prost(message, optional, tag = "4")]
    pub code: ::core::option::Option<AccountCode>,
    /// Account nonce.
    #[prost(uint64, tag = "5")]
    pub nonce: u64,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountStorageDelta {
    /// Items to be cleared in the account's storage.
    #[prost(bytes = "vec", tag = "1")]
    pub cleared_items: ::prost::alloc::vec::Vec<u8>,
    /// Vector of slots to be updated in the account's storage in the same order, as items.
    #[prost(bytes = "vec", tag = "2")]
    pub updated_storage_slots: ::prost::alloc::vec::Vec<u8>,
    /// Vector of items to be updated in the account's storage in the same order, as slots.
    #[prost(message, repeated, tag = "3")]
    pub updated_items: ::prost::alloc::vec::Vec<super::digest::Digest>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountVaultDelta {
    /// Assets to be added into the account's vault.
    #[prost(message, repeated, tag = "1")]
    pub added_assets: ::prost::alloc::vec::Vec<Asset>,
    /// Assets to be removed from the account's vault.
    #[prost(message, repeated, tag = "2")]
    pub removed_assets: ::prost::alloc::vec::Vec<Asset>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountDelta {
    /// Account's storage delta.
    #[prost(message, optional, tag = "1")]
    pub storage: ::core::option::Option<AccountStorageDelta>,
    /// Account's assets vault delta.
    #[prost(message, optional, tag = "2")]
    pub vault: ::core::option::Option<AccountVaultDelta>,
    /// Account's new nonce.
    #[prost(uint64, optional, tag = "3")]
    pub nonce: ::core::option::Option<u64>,
}
#[derive(Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountDetails {
    /// Details enumeration for public accounts.
    #[prost(oneof = "account_details::Details", tags = "1, 2")]
    pub details: ::core::option::Option<account_details::Details>,
}
/// Nested message and enum types in `AccountDetails`.
pub mod account_details {
    /// Details enumeration for public accounts.
    #[derive(Eq, PartialOrd, Ord, Hash)]
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Details {
        #[prost(message, tag = "1")]
        Full(super::AccountFullDetails),
        #[prost(message, tag = "2")]
        Delta(super::AccountDelta),
    }
}
