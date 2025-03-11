use std::path::Path;

use rusqlite::vtab::array;

use crate::db::transaction::Transaction;

pub struct Connection {
    inner: rusqlite::Connection,
}

impl Connection {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        rusqlite::Connection::open(path).and_then(Self::new)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        rusqlite::Connection::open_in_memory().and_then(Self::new)
    }

    fn new(inner: rusqlite::Connection) -> rusqlite::Result<Self> {
        // Feature used to support `IN` and `NOT IN` queries. We need to load
        // this module for every connection we create to the DB to support the
        // queries we want to run
        array::load_module(&inner)?;

        Ok(Self { inner })
    }

    pub(crate) fn inner(&self) -> &rusqlite::Connection {
        &self.inner
    }

    pub(crate) fn inner_mut(&mut self) -> &mut rusqlite::Connection {
        &mut self.inner
    }

    #[inline]
    pub fn transaction(&mut self) -> rusqlite::Result<Transaction<'_>> {
        self.inner.transaction().map(Transaction::new)
    }
}
