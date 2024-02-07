use std::fmt::{Debug, Display, Formatter};

use miden_crypto::merkle::MerklePath;
use miden_objects::{accounts::AccountId, Digest, Digest as RpoDigest};

use crate::{
    errors::ParseError,
    generated::{account, requests, responses},
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

impl From<(AccountId, RpoDigest)> for requests::AccountUpdate {
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

impl TryFrom<responses::AccountBlockInputRecord> for AccountInputRecord {
    type Error = ParseError;

    fn try_from(
        account_input_record: responses::AccountBlockInputRecord
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            account_id: account_input_record
                .account_id
                .ok_or(ParseError::ProtobufMissingData)?
                .try_into()?,
            account_hash: account_input_record
                .account_hash
                .ok_or(ParseError::ProtobufMissingData)?
                .try_into()?,
            proof: account_input_record.proof.ok_or(ParseError::ProtobufMissingData)?.try_into()?,
        })
    }
}
