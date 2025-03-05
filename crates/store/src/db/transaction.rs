pub struct Transaction<'conn> {
    inner: rusqlite::Transaction<'conn>,
}

impl<'conn> Transaction<'conn> {
    pub(super) fn new(inner: rusqlite::Transaction<'conn>) -> Self {
        Self { inner }
    }

    #[cfg(feature = "explain-query-plans")]
    pub fn inner(&self) -> &rusqlite::Transaction<'conn> {
        &self.inner
    }

    #[inline]
    pub fn commit(self) -> rusqlite::Result<()> {
        self.inner.commit()
    }
}

#[cfg(not(feature = "explain-query-plans"))]
impl Transaction<'_> {
    #[inline]
    pub fn prepare_cached(&self, sql: &str) -> rusqlite::Result<rusqlite::CachedStatement<'_>> {
        self.inner.prepare_cached(sql)
    }
}
