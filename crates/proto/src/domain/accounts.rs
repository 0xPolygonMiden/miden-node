use std::fmt::{Debug, Display, Formatter};

use miden_node_utils::formatting::format_opt;
use miden_objects::{
    accounts::{Account, AccountHeader, AccountId},
    crypto::{hash::rpo::RpoDigest, merkle::MerklePath},
    utils::{Deserializable, Serializable},
    Digest,
};

use super::try_convert;
use crate::{
    errors::{ConversionError, MissingFieldHelper},
    generated::{
        account as proto,
        requests::get_account_proofs_request,
        responses::{AccountBlockInputRecord, AccountTransactionInputRecord},
    },
};

// ACCOUNT ID
// ================================================================================================

impl Display for proto::AccountId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x")?;
        for byte in &self.id {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl Debug for proto::AccountId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

// INTO PROTO ACCOUNT ID
// ------------------------------------------------------------------------------------------------

impl From<&AccountId> for proto::AccountId {
    fn from(account_id: &AccountId) -> Self {
        (*account_id).into()
    }
}

impl From<AccountId> for proto::AccountId {
    fn from(account_id: AccountId) -> Self {
        Self { id: account_id.to_bytes() }
    }
}

// FROM PROTO ACCOUNT ID
// ------------------------------------------------------------------------------------------------

impl TryFrom<proto::AccountId> for AccountId {
    type Error = ConversionError;

    fn try_from(account_id: proto::AccountId) -> Result<Self, Self::Error> {
        AccountId::read_from_bytes(&account_id.id).map_err(|_| ConversionError::NotAValidFelt)
    }
}

// ACCOUNT UPDATE
// ================================================================================================

#[derive(Debug, PartialEq)]
pub struct AccountSummary {
    pub account_id: AccountId,
    pub account_hash: RpoDigest,
    pub block_num: u32,
}

impl From<&AccountSummary> for proto::AccountSummary {
    fn from(update: &AccountSummary) -> Self {
        Self {
            account_id: Some(update.account_id.into()),
            account_hash: Some(update.account_hash.into()),
            block_num: update.block_num,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct AccountInfo {
    pub summary: AccountSummary,
    pub details: Option<Account>,
}

impl From<&AccountInfo> for proto::AccountInfo {
    fn from(AccountInfo { summary, details }: &AccountInfo) -> Self {
        Self {
            summary: Some(summary.into()),
            details: details.as_ref().map(|account| account.to_bytes()),
        }
    }
}

// ACCOUNT STORAGE REQUEST
// ================================================================================================

/// Represents a request for an account proof alongside specific storage data.
pub struct AccountProofRequest {
    pub account_id: AccountId,
    pub storage_requests: Vec<StorageMapKeysProof>,
}

impl TryInto<AccountProofRequest> for get_account_proofs_request::AccountRequest {
    type Error = ConversionError;

    fn try_into(self) -> Result<AccountProofRequest, Self::Error> {
        let get_account_proofs_request::AccountRequest { account_id, storage_requests } = self;

        Ok(AccountProofRequest {
            account_id: account_id
                .clone()
                .ok_or(get_account_proofs_request::AccountRequest::missing_field(stringify!(
                    account_id
                )))?
                .try_into()?,
            storage_requests: try_convert(storage_requests)?,
        })
    }
}

/// Represents a request for an account's storage map values and its proof of existence.
pub struct StorageMapKeysProof {
    /// Index of the storage map
    pub storage_index: u8,
    /// List of requested keys in the map
    pub storage_keys: Vec<Digest>,
}

impl TryInto<StorageMapKeysProof> for get_account_proofs_request::StorageRequest {
    type Error = ConversionError;

    fn try_into(self) -> Result<StorageMapKeysProof, Self::Error> {
        let get_account_proofs_request::StorageRequest { storage_slot_index, map_keys } = self;

        Ok(StorageMapKeysProof {
            storage_index: storage_slot_index.try_into()?,
            storage_keys: try_convert(map_keys)?,
        })
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
            proof: Some(Into::into(&from.proof)),
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
                .as_ref()
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
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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

impl From<AccountHeader> for proto::AccountHeader {
    fn from(from: AccountHeader) -> Self {
        Self {
            vault_root: Some(from.vault_root().into()),
            storage_commitment: Some(from.storage_commitment().into()),
            code_commitment: Some(from.code_commitment().into()),
            nonce: from.nonce().into(),
        }
    }
}

impl TryFrom<AccountTransactionInputRecord> for AccountState {
    type Error = ConversionError;

    fn try_from(from: AccountTransactionInputRecord) -> Result<Self, Self::Error> {
        let account_id = from
            .account_id
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

        Ok(Self { account_id, account_hash })
    }
}
