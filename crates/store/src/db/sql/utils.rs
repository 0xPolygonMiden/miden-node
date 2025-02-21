use miden_node_proto::domain::account::AccountSummary;
use miden_objects::{
    block::BlockNumber, crypto::hash::rpo::RpoDigest, note::Nullifier, utils::Deserializable,
};
use rusqlite::{
    params, types::Value, AndThenRows, CachedStatement, Connection, OptionalExtension, Params, Row,
};

/// Returns the high 16 bits of the provided nullifier.
pub fn get_nullifier_prefix(nullifier: &Nullifier) -> u32 {
    (nullifier.most_significant_felt().as_int() >> 48) as u32
}

/// Checks if a table exists in the database.
pub fn table_exists(conn: &Connection, table_name: &str) -> rusqlite::Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = $1",
            params![table_name],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Returns the schema version of the database.
pub fn schema_version(conn: &Connection) -> rusqlite::Result<usize> {
    conn.query_row("SELECT * FROM pragma_schema_version", [], |row| row.get(0))
}

/// Auxiliary macro which substitutes `$src` token by `$dst` expression.
macro_rules! subst {
    ($src:tt, $dst:expr) => {
        $dst
    };
}

pub(crate) use subst;

/// Generates a simple insert SQL statement with parameters for the provided table name and fields.
/// Supports optional conflict resolution (adding "| replace" or "| ignore" at the end will generate
/// "OR REPLACE" and "OR IGNORE", correspondingly).
///
/// # Usage:
///
/// `insert_sql!(users { id, first_name, last_name, age } | replace);`
///
/// which generates:
/// "INSERT OR REPLACE INTO users (id, `first_name`, `last_name`, age) VALUES (?, ?, ?, ?)"
macro_rules! insert_sql {
    ($table:ident { $first_field:ident $(, $($field:ident),+)? $(,)? } $(, $on_conflict:expr)?) => {
        concat!(
            stringify!(INSERT $(OR $on_conflict)? INTO $table),
            " (",
            stringify!($first_field),
            $($(concat!(", ", stringify!($field))),+ ,)?
            ") VALUES (",
            subst!($first_field, "?"),
            $($(subst!($field, ", ?")),+ ,)?
            ")"
        )
    };

    ($table:ident { $first_field:ident $(, $($field:ident),+)? $(,)? } | replace) => {
        insert_sql!($table { $first_field, $($($field),+)? }, REPLACE)
    };

    ($table:ident { $first_field:ident $(, $($field:ident),+)? $(,)? } | ignore) => {
        insert_sql!($table { $first_field, $($($field),+)? }, IGNORE)
    };
}

pub(crate) use insert_sql;

/// Converts a `u64` into a [Value].
///
/// Sqlite uses `i64` as its internal representation format. Note that the `as` operator performs a
/// lossless conversion from `u64` to `i64`.
pub fn u64_to_value(v: u64) -> Value {
    #[allow(
        clippy::cast_possible_wrap,
        reason = "We store u64 as i64 as sqlite only allows the latter."
    )]
    Value::Integer(v as i64)
}

/// Gets a `u64` value from the database.
///
/// Sqlite uses `i64` as its internal representation format, and so when retrieving
/// we need to make sure we cast as `u64` to get the original value
pub fn column_value_as_u64<I: rusqlite::RowIndex>(
    row: &Row<'_>,
    index: I,
) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    #[allow(
        clippy::cast_sign_loss,
        reason = "We store u64 as i64 as sqlite only allows the latter."
    )]
    Ok(value as u64)
}

/// Gets a `BlockNum` value from the database.
pub fn read_block_number<I: rusqlite::RowIndex>(
    row: &Row<'_>,
    index: I,
) -> rusqlite::Result<BlockNumber> {
    let value: u32 = row.get(index)?;
    Ok(value.into())
}

/// Gets a blob value from the database and tries to deserialize it into the necessary type.
pub fn read_from_blob_column<I, T>(row: &Row<'_>, index: I) -> rusqlite::Result<T>
where
    I: rusqlite::RowIndex + Copy + Into<usize>,
    T: Deserializable,
{
    let value = row.get_ref(index)?.as_blob()?;

    T::read_from_bytes(value).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(
            index.into(),
            rusqlite::types::Type::Blob,
            Box::new(err),
        )
    })
}

/// Constructs `AccountSummary` from the row of `accounts` table.
///
/// Note: field ordering must be the same, as in `accounts` table!
pub fn account_summary_from_row(row: &Row<'_>) -> crate::db::Result<AccountSummary> {
    let account_id = read_from_blob_column(row, 0)?;
    let account_hash_data = row.get_ref(1)?.as_blob()?;
    let account_hash = RpoDigest::read_from_bytes(account_hash_data)?;
    let block_num = read_block_number(row, 2)?;

    Ok(AccountSummary { account_id, account_hash, block_num })
}

/// Cached SQL query statement augmented with params and row-to-entity mapping function.
pub struct AugmentedStatement<'conn, P, F> {
    statement: CachedStatement<'conn>,
    params: P,
    mapper: F,
}

impl<'conn, P, T, F> AugmentedStatement<'conn, P, F>
where
    P: Params + Clone,
    F: FnMut(&Row<'_>) -> crate::db::Result<T> + Clone,
{
    pub fn new(statement: CachedStatement<'conn>, params: P, mapper: F) -> Self {
        Self { statement, params, mapper }
    }

    pub fn query(&mut self) -> rusqlite::Result<AndThenRows<F>> {
        self.statement.query_and_then(self.params.clone(), self.mapper.clone())
    }
}
