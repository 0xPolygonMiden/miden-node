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

#[cfg(not(feature = "explain-query-plans"))]
impl Connection {
    #[inline]
    pub fn prepare_cached(&self, sql: &str) -> rusqlite::Result<rusqlite::CachedStatement<'_>> {
        self.inner.prepare_cached(sql)
    }

    #[inline]
    pub fn execute<P: rusqlite::Params>(&self, sql: &str, params: P) -> rusqlite::Result<usize> {
        self.inner.execute(sql, params)
    }

    #[inline]
    pub fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> rusqlite::Result<T>
    where
        P: rusqlite::Params,
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        self.inner.query_row(sql, params, f)
    }
}
