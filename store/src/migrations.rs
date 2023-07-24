use once_cell::sync::Lazy;
use rusqlite_migration::{Migrations, M};

pub static MIGRATIONS: Lazy<Migrations> = Lazy::new(|| {
    Migrations::new(vec![M::up(
        "CREATE TABLE nullifiers (id INTEGER PRIMARY KEY, nullifier BLOB, block_number INTEGER);",
    )])
});

#[test]
fn migrations_test() {
    assert!(MIGRATIONS.validate().is_ok());
}
