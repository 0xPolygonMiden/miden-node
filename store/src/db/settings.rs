use rusqlite::{params, Connection, OptionalExtension};

use crate::db::sql::table_exists;

pub struct Settings;

const DB_TABLE_NAME: &str = "settings";

impl Settings {
    pub fn exists(conn: &Connection) -> rusqlite::Result<bool> {
        table_exists(conn, DB_TABLE_NAME)
    }

    pub fn get_value(conn: &Connection, name: &str) -> rusqlite::Result<Option<Vec<u8>>> {
        conn.query_row(
            &format!("SELECT value FROM {DB_TABLE_NAME} WHERE name = $1"),
            params![name],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
    }

    pub fn set_value(conn: &Connection, name: &str, value: &[u8]) -> rusqlite::Result<()> {
        let count = conn.execute(
            &format!("INSERT OR REPLACE INTO {DB_TABLE_NAME} (name, value) VALUES (?, ?)"),
            params![name, value],
        )?;

        debug_assert_eq!(count, 1);

        Ok(())
    }
}
