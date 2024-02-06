use std::fmt::{Debug, Display, Formatter};

use miden_crypto::merkle::MerklePath;
use miden_node_utils::formatting::format_opt;
use miden_objects::{accounts::AccountId, Digest, Digest as RpoDigest};

use crate::{
    errors::{MissingFieldHelper, ParseError},
    generated::{
        account,
        requests::AccountUpdate,
        responses::{
            AccountBlockInputRecord,
            AccountTransactionInputRecord as AccountTransactionInputRecordPB,
        },
    },
};

// AccountId formatting
// ================================================================================================

impl Display for account::AccountId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_fmt(format_args!("0x{:x}", self.id))
    }
}

impl Debug for account::AccountId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

// INTO AccountId
// ================================================================================================

impl From<u64> for account::AccountId {
    fn from(value: u64) -> Self {
        account::AccountId { id: value }
    }
}

impl From<AccountId> for account::AccountId {
    fn from(account_id: AccountId) -> Self {
        Self {
            id: account_id.into(),
        }
    }
}

// FROM AccountId
// ================================================================================================

impl From<account::AccountId> for u64 {
    fn from(value: account::AccountId) -> Self {
        value.id
    }
}

impl TryFrom<account::AccountId> for AccountId {
    type Error = ParseError;

    fn try_from(account_id: account::AccountId) -> Result<Self, Self::Error> {
        account_id.id.try_into().map_err(|_| ParseError::NotAValidFelt)
    }
}

// INTO AccountUpdate
// ================================================================================================

impl From<(AccountId, RpoDigest)> for AccountUpdate {
    fn from((account_id, account_hash): (AccountId, RpoDigest)) -> Self {
        Self {
            account_id: Some(account_id.into()),
            account_hash: Some(account_hash.into()),
        }
    }
}

// AccountInputRecord
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
    type Error = ParseError;

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

impl From<AccountState> for AccountTransactionInputRecordPB {
    fn from(from: AccountState) -> Self {
        Self {
            account_id: Some(from.account_id.into()),
            account_hash: from.account_hash.map(Into::into),
        }
    }
}

impl TryFrom<AccountTransactionInputRecordPB> for AccountState {
    type Error = ParseError;

    fn try_from(from: AccountTransactionInputRecordPB) -> Result<Self, Self::Error> {
        let account_id = from
            .account_id
            .clone()
            .ok_or(AccountTransactionInputRecordPB::missing_field(stringify!(account_id)))?
            .try_into()?;

        let account_hash = from
            .account_hash
            .ok_or(AccountTransactionInputRecordPB::missing_field(stringify!(account_hash)))?
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
    type Error = ParseError;

    fn try_from(value: AccountUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            account_id: value
                .account_id
                .ok_or(AccountUpdate::missing_field(stringify!(account_id)))?
                .try_into()?,
            account_hash: Some(
                value
                    .account_hash
                    .ok_or(AccountUpdate::missing_field(stringify!(account_hash)))?
                    .try_into()?,
            ),
        })
    }
}

impl TryFrom<&AccountUpdate> for AccountState {
    type Error = ParseError;

    fn try_from(value: &AccountUpdate) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}