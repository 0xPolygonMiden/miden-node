use rusqlite::{params, types::FromSql, Connection, OptionalExtension, Result, ToSql};

use crate::db::sql::table_exists;

pub struct Settings;

impl Settings {
    pub fn exists(conn: &Connection) -> Result<bool> {
        table_exists(conn, "settings")
    }

    pub fn get_value<T: FromSql>(conn: &Connection, name: &str) -> Result<Option<T>> {
        conn.query_row("SELECT value FROM settings WHERE name = $1", params![name], |row| {
            row.get/*::<_, T>*/(0)
        })
        .optional()
    }

    pub fn set_value<T: ToSql>(conn: &Connection, name: &str, value: &T) -> Result<()> {
        let count = conn.execute(
            "INSERT OR REPLACE INTO settings (name, value) VALUES (?, ?)",
            params![name, value],
        )?;

        debug_assert_eq!(count, 1);

        Ok(())
    }
}
