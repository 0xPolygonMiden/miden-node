use std::path::PathBuf;

use deadpool::{
    managed::{Manager, Metrics, RecycleResult},
    Runtime,
};

use crate::{db::connection::Connection, errors::DatabaseError, SQL_STATEMENT_CACHE_CAPACITY};

deadpool::managed_reexports!(
    "miden-node-store",
    SqlitePoolManager,
    deadpool::managed::Object<SqlitePoolManager>,
    rusqlite::Error,
    DatabaseError
);

const RUNTIME: Runtime = Runtime::Tokio1;

pub struct SqlitePoolManager {
    database_path: PathBuf,
}

/// SQLite connection pool manager for optional query plan rendering.
impl SqlitePoolManager {
    pub fn new(database_path: PathBuf) -> Self {
        Self { database_path }
    }

    fn new_connection(&self) -> rusqlite::Result<Connection> {
        let conn = Connection::open(&self.database_path)?;

        tracing::info!("conn open");
        // Increase the statement cache size.
        conn.set_prepared_statement_cache_capacity(SQL_STATEMENT_CACHE_CAPACITY);

        tracing::info!("conn cache");
        // Enable the WAL mode. This allows concurrent reads while the
        // transaction is being written, this is required for proper
        // synchronization of the servers in-memory and on-disk representations
        // (see [State::apply_block])
        conn.pragma_update(None, "journal_mode", "WAL")?;
        tracing::info!("conn WAL");

        // Enable foreign key checks.
        conn.pragma_update(None, "foreign_keys", "ON")?;
        tracing::info!("conn FK");

        Ok(conn)
    }
}

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
