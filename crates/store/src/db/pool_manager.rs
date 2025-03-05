use std::path::PathBuf;

use deadpool::Runtime;

use crate::errors::DatabaseError;

deadpool::managed_reexports!(
    "miden-node-store",
    SqlitePoolManager,
    deadpool::managed::Object<SqlitePoolManager>,
    rusqlite::Error,
    DatabaseError
);

use crate::SQL_STATEMENT_CACHE_CAPACITY;

const RUNTIME: Runtime = Runtime::Tokio1;

pub struct SqlitePoolManager {
    database_path: PathBuf,
}

impl SqlitePoolManager {
    pub fn new(database_path: PathBuf) -> Self {
        Self { database_path }
    }

    pub fn new_connection(&self) -> rusqlite::Result<Connection> {
        let conn = Connection::open(&self.database_path)?;
        let inner = conn.inner();

        // Increase the statement cache size.
        inner.set_prepared_statement_cache_capacity(SQL_STATEMENT_CACHE_CAPACITY);

        // Enable the WAL mode. This allows concurrent reads while the
        // transaction is being written, this is required for proper
        // synchronization of the servers in-memory and on-disk representations
        // (see [State::apply_block])
        inner.execute("PRAGMA journal_mode = WAL;", ())?;

        // Enable foreign key checks.
        inner.execute("PRAGMA foreign_keys = ON;", ())?;

        Ok(conn)
    }
}

use deadpool::managed::{Manager, Metrics, RecycleResult};

use crate::db::connection::Connection;

impl Manager for SqlitePoolManager {
    type Type = deadpool_sync::SyncWrapper<Connection>;
    type Error = rusqlite::Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let conn = self.new_connection();
        deadpool_sync::SyncWrapper::new(RUNTIME, move || conn).await
    }

    async fn recycle(&self, _: &mut Self::Type, _: &Metrics) -> RecycleResult<Self::Error> {
        Ok(())
    }
}
