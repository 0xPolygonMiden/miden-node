use std::fmt::{Debug, Display, Formatter};

use miden_node_utils::formatting::format_opt;
use miden_objects::{
    accounts::{Account, AccountId},
    crypto::{hash::rpo::RpoDigest, merkle::MerklePath},
    utils::Serializable,
    Digest,
};

use crate::{
    errors::{ConversionError, MissingFieldHelper},
    generated::{
        account::{
            AccountId as AccountIdPb, AccountInfo as AccountInfoPb,
            AccountSummary as AccountSummaryPb,
        },
        responses::{AccountBlockInputRecord, AccountTransactionInputRecord},
    },
};

use super::transaction::TransactionInfo;

// ACCOUNT ID
// ================================================================================================

impl Display for AccountIdPb {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("0x{:x}", self.id))
    }
}

impl Debug for AccountIdPb {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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

impl From<&AccountId> for AccountIdPb {
    fn from(account_id: &AccountId) -> Self {
        (*account_id).into()
    }
}

impl From<AccountId> for AccountIdPb {
    fn from(account_id: AccountId) -> Self {
        Self { id: account_id.into() }
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

// ACCOUNT UPDATE
// ================================================================================================

// An account update represents both the account hash change and the transactions executed by that
// account
#[derive(Debug, PartialEq)]
pub struct AccountUpdate {
    pub account_summary: AccountSummary,
    pub transactions: Vec<TransactionInfo>,
}

// ACCOUNT SUMMARY
// ================================================================================================

#[derive(Debug, PartialEq)]
pub struct AccountSummary {
    pub account_id: AccountId,
    pub account_hash: RpoDigest,
    pub block_num: u32,
}

impl From<&AccountSummary> for AccountSummaryPb {
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

impl From<&AccountInfo> for AccountInfoPb {
    fn from(AccountInfo { summary, details }: &AccountInfo) -> Self {
        Self {
            summary: Some(summary.into()),
            details: details.as_ref().map(|account| account.to_bytes()),
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

        Ok(Self { account_id, account_hash })
    }
}
