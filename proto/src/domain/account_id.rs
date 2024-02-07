use miden_objects::accounts::AccountId;

use crate::{account, errors::ParseError};

// INTO
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

// FROM
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
