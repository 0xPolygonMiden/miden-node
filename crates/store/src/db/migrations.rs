use miden_objects::{
    crypto::hash::blake::{Blake3Digest, Blake3_160},
    utils::{Deserializable, Serializable},
};
use once_cell::sync::Lazy;
use rusqlite::Connection;
use rusqlite_migration::{Migrations, SchemaVersion, M};
use tracing::{debug, info, instrument};

use crate::{
    db::{settings::Settings, sql::schema_version},
    errors::DatabaseError,
    COMPONENT,
};

type Hash = Blake3Digest<20>;

const MIGRATION_SCRIPTS: [&str; 1] = [include_str!("migrations/001-init.sql")];
static MIGRATION_HASHES: Lazy<Vec<Hash>> = Lazy::new(compute_migration_hashes);
static MIGRATIONS: Lazy<Migrations> = Lazy::new(prepare_migrations);

fn up(s: &'static str) -> M<'static> {
    M::up(s).foreign_key_check()
}

const DB_MIGRATION_HASH_FIELD: &str = "db-migration-hash";
const DB_SCHEMA_VERSION_FIELD: &str = "db-schema-version";

#[instrument(target = "miden-store", skip_all, err)]
pub fn apply_migrations(conn: &mut Connection) -> super::Result<()> {
    let version_before = MIGRATIONS.current_version(conn)?;

    info!(target: COMPONENT, version_before = %version_before, "Running database migrations");

    if let SchemaVersion::Inside(ver) = version_before {
        if !Settings::exists(conn)? {
            return Err(DatabaseError::UnsupportedDatabaseVersion);
        }

        let last_schema_version = last_schema_version(conn)?;
        let current_schema_version = schema_version(conn)?;

        if last_schema_version != current_schema_version {
            return Err(DatabaseError::UnsupportedDatabaseVersion);
        }

        let expected_hash = MIGRATION_HASHES[ver.get() - 1].to_bytes();
        let actual_hash = Settings::get_value(conn, DB_MIGRATION_HASH_FIELD)?;

        debug!(
            target: COMPONENT,
            expected_hash = %hex::encode(&expected_hash),
            actual_hash = ?actual_hash.as_ref().map(hex::encode),
            "Comparing migration hashes",
        );

        if actual_hash != Some(expected_hash) {
            return Err(DatabaseError::UnsupportedDatabaseVersion);
        }
    }

    MIGRATIONS.to_latest(conn).map_err(DatabaseError::MigrationError)?;

    if version_before != MIGRATIONS.current_version(conn)? {
        let last_hash = MIGRATION_HASHES[MIGRATION_HASHES.len() - 1].to_bytes();
        debug!(target: COMPONENT, new_hash = %hex::encode(&last_hash), "Updating migration hash in settings table");
        Settings::set_value(conn, DB_MIGRATION_HASH_FIELD, &last_hash)?;
    }

    let new_schema_version = schema_version(conn)?;
    Settings::set_value(conn, DB_SCHEMA_VERSION_FIELD, &new_schema_version.to_bytes())?;

    Ok(())
}

fn last_schema_version(conn: &Connection) -> super::Result<u32> {
    let Some(schema_version) = Settings::get_value(conn, DB_SCHEMA_VERSION_FIELD)? else {
        return Err(DatabaseError::UnsupportedDatabaseVersion);
    };

    u32::read_from_bytes(&schema_version).map_err(Into::into)
}

fn prepare_migrations() -> Migrations<'static> {
    Migrations::new(MIGRATION_SCRIPTS.map(up).to_vec())
}

fn compute_migration_hashes() -> Vec<Hash> {
    let mut accumulator = Hash::default();
    MIGRATION_SCRIPTS
        .iter()
        .map(|sql| {
            let script_hash = Blake3_160::hash(preprocess_sql(sql).as_bytes());
            accumulator = Blake3_160::merge(&[accumulator, script_hash]);
            accumulator
        })
        .collect()
}

fn preprocess_sql(sql: &str) -> String {
    // TODO: We can also remove all comments here (need to analyze the SQL script in order to remain comments
    //       in string literals).
    remove_spaces(sql)
}

fn remove_spaces(str: &str) -> String {
    str.chars().filter(|chr| !chr.is_whitespace()).collect()
}

#[test]
fn migrations_validate() {
    assert_eq!(MIGRATIONS.validate(), Ok(()));
}
