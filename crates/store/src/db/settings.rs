use rusqlite::{params, Connection, OptionalExtension};

use crate::db::sql::table_exists;

pub struct Settings;

impl Settings {
    pub fn exists(conn: &Connection) -> rusqlite::Result<bool> {
        table_exists(conn, "settings")
    }

    pub fn get_value(conn: &Connection, name: &str) -> rusqlite::Result<Option<Vec<u8>>> {
        conn.query_row("SELECT value FROM settings WHERE name = $1", params![name], |row| {
            row.get::<_, Vec<u8>>(0)
        })
        .optional()
    }

    pub fn set_value(conn: &Connection, name: &str, value: &[u8]) -> rusqlite::Result<()> {
        let count = conn.execute(
            "INSERT OR REPLACE INTO settings (name, value) VALUES (?, ?)",
            params![name, value],
        )?;

        debug_assert_eq!(count, 1);

        Ok(())
    }
}
