use std::fmt::{Debug, Display, Formatter};

pub type BlockNumber = u32;

#[derive(Clone, Copy)]
pub struct AccountId(pub u64);

impl From<u64> for AccountId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<AccountId> for u64 {
    fn from(value: AccountId) -> Self {
        value.0
    }
}

impl From<&u64> for AccountId {
    fn from(value: &u64) -> Self {
        Self(*value)
    }
}

impl From<&AccountId> for u64 {
    fn from(value: &AccountId) -> Self {
        value.0
    }
}

impl Display for AccountId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_fmt(format_args!("0x{:x}", self.0))
    }
}

impl Debug for AccountId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}
