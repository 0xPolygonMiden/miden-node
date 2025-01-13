use miden_node_proto::domain::accounts::{AccountInfo, AccountSummary};
use miden_objects::{
    accounts::{Account, AccountDelta, AccountId},
    crypto::hash::rpo::RpoDigest,
    notes::Nullifier,
    utils::Deserializable,
};
use rusqlite::{
    params,
    types::{Value, ValueRef},
    Connection, OptionalExtension,
};

use crate::errors::DatabaseError;

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
///
/// # Usage:
///
/// `insert_sql!(users { id, first_name, last_name, age });`
///
/// which generates:
/// "INSERT INTO users (id, first_name, last_name, age) VALUES (?, ?, ?, ?)"
macro_rules! insert_sql {
    ($table:ident { $first_field:ident $(, $($field:ident),+)? $(,)? }) => {
        concat!(
            stringify!(INSERT INTO $table),
            " (",
            stringify!($first_field),
            $($(concat!(", ", stringify!($field))),+ ,)?
            ") VALUES (",
            subst!($first_field, "?"),
            $($(subst!($field, ", ?")),+ ,)?
            ")"
        )
    };
}

pub(crate) use insert_sql;

/// Converts a `u64` into a [Value].
///
/// Sqlite uses `i64` as its internal representation format. Note that the `as` operator performs a
/// lossless conversion from `u64` to `i64`.
pub fn u64_to_value(v: u64) -> Value {
    Value::Integer(v as i64)
}

/// Gets a `u64` value from the database.
///
/// Sqlite uses `i64` as its internal representation format, and so when retrieving
/// we need to make sure we cast as `u64` to get the original value
pub fn column_value_as_u64<I: rusqlite::RowIndex>(
    row: &rusqlite::Row<'_>,
    index: I,
) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    Ok(value as u64)
}

/// Gets a blob value from the database and tries to deserialize it into the necessary type.
pub fn read_from_blob_column<I, T>(row: &rusqlite::Row<'_>, index: I) -> rusqlite::Result<T>
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

/// Gets an optional (nullable) blob value from the database and tries to deserialize it into
/// the necessary type.
pub fn read_nullable_from_blob_column<I, T>(
    row: &rusqlite::Row<'_>,
    index: I,
) -> rusqlite::Result<Option<T>>
where
    I: rusqlite::RowIndex + Copy + Into<usize>,
    T: Deserializable,
{
    row.get_ref(index)?
        .as_blob_or_null()?
        .map(|value| T::read_from_bytes(value))
        .transpose()
        .map_err(|err| {
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
pub fn account_summary_from_row(row: &rusqlite::Row<'_>) -> crate::db::Result<AccountSummary> {
    let account_id = read_from_blob_column(row, 0)?;
    let account_hash_data = row.get_ref(1)?.as_blob()?;
    let account_hash = RpoDigest::read_from_bytes(account_hash_data)?;
    let block_num = row.get(2)?;

    Ok(AccountSummary { account_id, account_hash, block_num })
}

/// Constructs `AccountInfo` from the row of `accounts` table.
///
/// Note: field ordering must be the same, as in `accounts` table!
pub fn account_info_from_row(row: &rusqlite::Row<'_>) -> crate::db::Result<AccountInfo> {
    let update = account_summary_from_row(row)?;

    let details = row.get_ref(3)?.as_blob_or_null()?;
    let details = details.map(Account::read_from_bytes).transpose()?;

    Ok(AccountInfo { summary: update, details })
}

/// Deserializes account and applies account delta.
pub fn apply_delta(
    account_id: AccountId,
    value: &ValueRef<'_>,
    delta: &AccountDelta,
    final_state_hash: &RpoDigest,
) -> crate::db::Result<Account, DatabaseError> {
    let account = value.as_blob_or_null()?;
    let account = account.map(Account::read_from_bytes).transpose()?;

    let Some(mut account) = account else {
        return Err(DatabaseError::AccountNotOnChain(account_id));
    };

    account.apply_delta(delta)?;

    let actual_hash = account.hash();
    if &actual_hash != final_state_hash {
        return Err(DatabaseError::AccountHashesMismatch {
            calculated: actual_hash,
            expected: *final_state_hash,
        });
    }

    Ok(account)
}
