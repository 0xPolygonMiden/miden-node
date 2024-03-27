use std::fmt::{Debug, Display, Formatter};

use miden_node_utils::formatting::format_opt;
use miden_objects::{
    accounts::{
        Account, AccountCode, AccountDelta, AccountId, AccountStorage, AccountStorageDelta,
        AccountVaultDelta,
    },
    assembly::{AstSerdeOptions, ModuleAst},
    assets::{Asset, AssetVault, FungibleAsset, NonFungibleAsset},
    crypto::{hash::rpo::RpoDigest, merkle::MerklePath},
    transaction::AccountDetails,
    utils::{Deserializable, Serializable},
    Digest, Felt, Word,
};

use crate::{
    convert,
    errors::{ConversionError, MissingFieldHelper},
    generated::{
        account::{
            account_details::{Details as DetailsPb, Details},
            asset::Asset as AssetEnumPb,
            AccountCode as AccountCodePb, AccountDelta as AccountDeltaPb,
            AccountDetails as AccountDetailsPb, AccountFullDetails as AccountFullDetailsPb,
            AccountId as AccountIdPb, AccountStorage as AccountStoragePb,
            AccountStorageDelta as AccountStorageDeltaPb, AccountVaultDelta as AccountVaultDeltaPb,
            Asset as AssetPb, AssetVault as AssetVaultPb, FungibleAsset as FungibleAssetPb,
            NonFungibleAsset as NonFungibleAssetPb,
        },
        requests::AccountUpdate,
        responses::{AccountBlockInputRecord, AccountTransactionInputRecord},
    },
    try_convert,
};

// ACCOUNT ID
// ================================================================================================

impl Display for AccountIdPb {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_fmt(format_args!("0x{:x}", self.id))
    }
}

impl Debug for AccountIdPb {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

// INTO PROTO ACCOUNT ID
// ------------------------------------------------------------------------------------------------

impl From<u64> for AccountIdPb {
    fn from(value: u64) -> Self {
        AccountIdPb { id: value }
    }
}

impl From<AccountId> for AccountIdPb {
    fn from(account_id: AccountId) -> Self {
        Self {
            id: account_id.into(),
        }
    }
}

// FROM PROTO ACCOUNT ID
// ------------------------------------------------------------------------------------------------

impl From<AccountIdPb> for u64 {
    fn from(value: AccountIdPb) -> Self {
        value.id
    }
}

impl TryFrom<AccountIdPb> for AccountId {
    type Error = ConversionError;

    fn try_from(account_id: AccountIdPb) -> Result<Self, Self::Error> {
        account_id.id.try_into().map_err(|_| ConversionError::NotAValidFelt)
    }
}

// INTO ACCOUNT DETAILS
// ================================================================================================

impl From<&FungibleAsset> for FungibleAssetPb {
    fn from(fungible: &FungibleAsset) -> Self {
        Self {
            faucet_id: Some(fungible.faucet_id().into()),
            amount: fungible.amount(),
        }
    }
}

impl From<&NonFungibleAsset> for NonFungibleAssetPb {
    fn from(non_fungible: &NonFungibleAsset) -> Self {
        Self {
            asset: Some(non_fungible.vault_key().into()),
        }
    }
}

impl From<&Asset> for AssetPb {
    fn from(asset: &Asset) -> Self {
        let asset = Some(match asset {
            Asset::Fungible(fungible) => AssetEnumPb::Fungible(fungible.into()),
            Asset::NonFungible(non_fungible) => AssetEnumPb::NonFungible(non_fungible.into()),
        });

        Self { asset }
    }
}

impl From<Asset> for AssetPb {
    fn from(asset: Asset) -> Self {
        asset.into()
    }
}

impl From<&AssetVault> for AssetVaultPb {
    fn from(vault: &AssetVault) -> Self {
        Self {
            assets: convert(vault.assets()),
        }
    }
}

impl From<&AccountStorage> for AccountStoragePb {
    fn from(storage: &AccountStorage) -> Self {
        Self {
            data: storage.to_bytes(),
        }
    }
}

impl From<&AccountCode> for AccountCodePb {
    fn from(code: &AccountCode) -> Self {
        Self {
            module: code.module().to_bytes(AstSerdeOptions::new(true)),
            procedures: convert(code.procedures()),
        }
    }
}

impl From<&Account> for AccountFullDetailsPb {
    fn from(account: &Account) -> Self {
        Self {
            id: Some(account.id().into()),
            vault: Some(account.vault().into()),
            storage: Some(account.storage().into()),
            code: Some(account.code().into()),
            nonce: account.nonce().as_int(),
        }
    }
}

impl From<&AccountStorageDelta> for AccountStorageDeltaPb {
    fn from(delta: &AccountStorageDelta) -> Self {
        Self {
            cleared_items: delta.cleared_items.clone(),
            updated_storage_slots: delta.updated_items.iter().map(|(slot, _)| *slot).collect(),
            updated_items: delta
                .updated_items
                .iter()
                .map(|(_, value)| Into::<RpoDigest>::into(value))
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<&AccountVaultDelta> for AccountVaultDeltaPb {
    fn from(delta: &AccountVaultDelta) -> Self {
        Self {
            added_assets: convert(delta.added_assets.iter()),
            removed_assets: convert(delta.removed_assets.iter()),
        }
    }
}

impl From<&AccountDelta> for AccountDeltaPb {
    fn from(delta: &AccountDelta) -> Self {
        Self {
            storage: Some(delta.storage().into()),
            vault: Some(delta.vault().into()),
            nonce: delta.nonce().as_ref().map(Felt::as_int),
        }
    }
}

impl From<&AccountDetails> for AccountDetailsPb {
    fn from(details: &AccountDetails) -> Self {
        let details = Some(match details {
            AccountDetails::Full(full) => DetailsPb::Full(full.into()),
            AccountDetails::Delta(delta) => DetailsPb::Delta(delta.into()),
        });

        Self { details }
    }
}

// FROM ACCOUNT DETAILS
// ================================================================================================

impl TryFrom<&FungibleAssetPb> for FungibleAsset {
    type Error = ConversionError;

    fn try_from(fungible: &FungibleAssetPb) -> Result<Self, Self::Error> {
        let faucet_id = fungible
            .faucet_id
            .clone()
            .ok_or(FungibleAssetPb::missing_field(stringify!(faucet_id)))?
            .try_into()?;

        Ok(Self::new(faucet_id, fungible.amount)?)
    }
}

impl TryFrom<&NonFungibleAssetPb> for NonFungibleAsset {
    type Error = ConversionError;

    fn try_from(non_fungible: &NonFungibleAssetPb) -> Result<Self, Self::Error> {
        let asset: Word = non_fungible
            .asset
            .clone()
            .ok_or(NonFungibleAssetPb::missing_field(stringify!(asset)))?
            .try_into()?;

        Ok(Self::try_from(asset)?)
    }
}

impl TryFrom<&AssetPb> for Asset {
    type Error = ConversionError;

    fn try_from(asset: &AssetPb) -> Result<Self, Self::Error> {
        let from = asset.asset.as_ref().ok_or(AssetPb::missing_field(stringify!(asset)))?;
        Ok(match from {
            AssetEnumPb::Fungible(fungible) => Asset::Fungible(fungible.try_into()?),
            AssetEnumPb::NonFungible(non_fungible) => Asset::NonFungible(non_fungible.try_into()?),
        })
    }
}

impl TryFrom<&AssetVaultPb> for AssetVault {
    type Error = ConversionError;

    fn try_from(vault: &AssetVaultPb) -> Result<Self, Self::Error> {
        let assets = try_convert(vault.assets.iter())?;

        Ok(Self::new(&assets)?)
    }
}

impl TryFrom<&AccountStoragePb> for AccountStorage {
    type Error = ConversionError;

    fn try_from(storage: &AccountStoragePb) -> Result<Self, Self::Error> {
        Ok(Self::read_from_bytes(&storage.data)?)
    }
}

impl TryFrom<&AccountCodePb> for AccountCode {
    type Error = ConversionError;

    fn try_from(code: &AccountCodePb) -> Result<Self, Self::Error> {
        let module = ModuleAst::from_bytes(&code.module)?;
        let procedures = try_convert(&code.procedures)?;

        Ok(Self::from_parts(module, procedures))
    }
}

impl TryFrom<&AccountFullDetailsPb> for Account {
    type Error = ConversionError;

    fn try_from(account: &AccountFullDetailsPb) -> Result<Self, Self::Error> {
        Ok(Self::new(
            account
                .id
                .clone()
                .ok_or(AccountFullDetailsPb::missing_field(stringify!(id)))?
                .try_into()?,
            account
                .vault
                .as_ref()
                .ok_or(AccountFullDetailsPb::missing_field(stringify!(vault)))?
                .try_into()?,
            account
                .storage
                .as_ref()
                .ok_or(AccountFullDetailsPb::missing_field(stringify!(storage)))?
                .try_into()?,
            account
                .code
                .as_ref()
                .ok_or(AccountFullDetailsPb::missing_field(stringify!(code)))?
                .try_into()?,
            Felt::new(account.nonce),
        ))
    }
}

impl TryFrom<&AccountStorageDeltaPb> for AccountStorageDelta {
    type Error = ConversionError;

    fn try_from(from: &AccountStorageDeltaPb) -> Result<Self, Self::Error> {
        let updated_items: Result<_, ConversionError> = from
            .updated_storage_slots
            .iter()
            .zip(from.updated_items.iter())
            .map(|(slot, value)| Ok((*slot, value.try_into()?)))
            .collect();
        let storage_delta = Self {
            cleared_items: from.cleared_items.clone(),
            updated_items: updated_items?,
        };

        storage_delta.validate()?;

        Ok(storage_delta)
    }
}

impl TryFrom<&AccountVaultDeltaPb> for AccountVaultDelta {
    type Error = ConversionError;

    fn try_from(delta: &AccountVaultDeltaPb) -> Result<Self, Self::Error> {
        Ok(Self {
            added_assets: try_convert(delta.added_assets.iter())?,
            removed_assets: try_convert(delta.removed_assets.iter())?,
        })
    }
}

impl TryFrom<&AccountDeltaPb> for AccountDelta {
    type Error = ConversionError;

    fn try_from(delta: &AccountDeltaPb) -> Result<Self, Self::Error> {
        Ok(Self::new(
            delta
                .storage
                .as_ref()
                .ok_or(AccountDeltaPb::missing_field(stringify!(storage)))?
                .try_into()?,
            delta
                .vault
                .as_ref()
                .ok_or(AccountDeltaPb::missing_field(stringify!(vault)))?
                .try_into()?,
            delta.nonce.map(Felt::new),
        )?)
    }
}

impl TryFrom<&DetailsPb> for AccountDetails {
    type Error = ConversionError;

    fn try_from(details: &DetailsPb) -> Result<Self, Self::Error> {
        Ok(match details {
            Details::Full(full) => AccountDetails::Full(full.try_into()?),
            Details::Delta(delta) => AccountDetails::Delta(delta.try_into()?),
        })
    }
}

impl TryFrom<&AccountDetailsPb> for AccountDetails {
    type Error = ConversionError;

    fn try_from(details: &AccountDetailsPb) -> Result<Self, Self::Error> {
        details
            .details
            .as_ref()
            .ok_or(AccountDetailsPb::missing_field(stringify!(details)))?
            .try_into()
    }
}

// INTO ACCOUNT UPDATE
// ================================================================================================

impl From<(AccountId, Option<AccountDetails>, Digest)> for AccountUpdate {
    fn from(
        (account_id, details, account_hash): (AccountId, Option<AccountDetails>, Digest)
    ) -> Self {
        Self {
            account_id: Some(account_id.into()),
            account_hash: Some(account_hash.into()),
            details: details.as_ref().map(Into::into),
        }
    }
}

// ACCOUNT INPUT RECORD
// ================================================================================================

#[derive(Clone, Debug)]
pub struct AccountInputRecord {
    pub account_id: AccountId,
    pub account_hash: Digest,
    pub proof: MerklePath,
}

impl From<AccountInputRecord> for AccountBlockInputRecord {
    fn from(from: AccountInputRecord) -> Self {
        Self {
            account_id: Some(from.account_id.into()),
            account_hash: Some(from.account_hash.into()),
            proof: Some(from.proof.into()),
        }
    }
}

impl TryFrom<AccountBlockInputRecord> for AccountInputRecord {
    type Error = ConversionError;

    fn try_from(account_input_record: AccountBlockInputRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            account_id: account_input_record
                .account_id
                .ok_or(AccountBlockInputRecord::missing_field(stringify!(account_id)))?
                .try_into()?,
            account_hash: account_input_record
                .account_hash
                .ok_or(AccountBlockInputRecord::missing_field(stringify!(account_hash)))?
                .try_into()?,
            proof: account_input_record
                .proof
                .ok_or(AccountBlockInputRecord::missing_field(stringify!(proof)))?
                .try_into()?,
        })
    }
}

// ACCOUNT STATE
// ================================================================================================

/// Information needed from the store to verify account in transaction.
#[derive(Debug)]
pub struct AccountState {
    /// Account ID
    pub account_id: AccountId,
    /// The account hash in the store corresponding to tx's account ID
    pub account_hash: Option<Digest>,
}

impl Display for AccountState {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ account_id: {}, account_hash: {} }}",
            self.account_id,
            format_opt(self.account_hash.as_ref()),
        ))
    }
}

impl From<AccountState> for AccountTransactionInputRecord {
    fn from(from: AccountState) -> Self {
        Self {
            account_id: Some(from.account_id.into()),
            account_hash: from.account_hash.map(Into::into),
        }
    }
}

impl TryFrom<AccountTransactionInputRecord> for AccountState {
    type Error = ConversionError;

    fn try_from(from: AccountTransactionInputRecord) -> Result<Self, Self::Error> {
        let account_id = from
            .account_id
            .clone()
            .ok_or(AccountTransactionInputRecord::missing_field(stringify!(account_id)))?
            .try_into()?;

        let account_hash = from
            .account_hash
            .ok_or(AccountTransactionInputRecord::missing_field(stringify!(account_hash)))?
            .try_into()?;

        // If the hash is equal to `Digest::default()`, it signifies that this is a new account
        // which is not yet present in the Store.
        let account_hash = if account_hash == Digest::default() {
            None
        } else {
            Some(account_hash)
        };

        Ok(Self {
            account_id,
            account_hash,
        })
    }
}

impl TryFrom<AccountUpdate> for AccountState {
    type Error = ConversionError;

    fn try_from(value: AccountUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            account_id: value
                .account_id
                .ok_or(AccountUpdate::missing_field(stringify!(account_id)))?
                .try_into()?,
            account_hash: value.account_hash.map(TryInto::try_into).transpose()?,
        })
    }
}

impl TryFrom<&AccountUpdate> for AccountState {
    type Error = ConversionError;

    fn try_from(value: &AccountUpdate) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}
