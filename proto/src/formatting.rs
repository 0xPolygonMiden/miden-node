use std::fmt::{Debug, Display, Formatter};

use hex::ToHex;

use crate::{account::AccountId, digest::Digest};

impl Display for AccountId {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_fmt(format_args!("0x{:x}", self.id))
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

impl Display for Digest {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str(&self.encode_hex::<String>())
    }
}

impl Debug for Digest {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}
