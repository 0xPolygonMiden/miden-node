use std::ops::{Deref, DerefMut};

/// SQLite transaction wrapper for optional query plan rendering.
pub struct Transaction<'conn> {
    inner: rusqlite::Transaction<'conn>,
}

impl<'conn> Transaction<'conn> {
    pub(super) fn new(inner: rusqlite::Transaction<'conn>) -> Self {
        Self { inner }
    }

    #[inline]
    pub fn commit(self) -> rusqlite::Result<()> {
        self.inner.commit()
    }
}

impl<'conn> Deref for Transaction<'conn> {
    type Target = rusqlite::Transaction<'conn>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Transaction<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Transaction<'_> {
    #[inline]
    pub fn prepare_cached(&self, sql: &str) -> rusqlite::Result<rusqlite::CachedStatement<'_>> {
        #[cfg(feature = "explain-query-plans")]
        self.explain_query_plan(sql)?;

        self.inner.prepare_cached(sql)
    }
}
