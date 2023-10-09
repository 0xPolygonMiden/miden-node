use once_cell::sync::Lazy;
use rusqlite_migration::{Migrations, M};

pub static MIGRATIONS: Lazy<Migrations> = Lazy::new(|| {
    Migrations::new(vec![M::up(
        "
        CREATE TABLE
            block_header
        (
            block_num INTEGER NOT NULL,
            block_header BLOB NOT NULL,

            PRIMARY KEY (block_num),
            CONSTRAINT block_header_block_num_positive CHECK (block_num >= 0)
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE
            nullifiers
        (
            nullifier BLOB NOT NULL,
            block_number INTEGER NOT NULL,

            PRIMARY KEY (nullifier),
            CONSTRAINT nullifiers_nullifier_valid_digest CHECK (length(nullifier) = 32),
            CONSTRAINT nullifiers_block_number_positive CHECK (block_number >= 0),
            FOREIGN KEY (block_number) REFERENCES block_header (block_num)
        ) STRICT, WITHOUT ROWID;
        ",
    )])
});

#[test]
fn migrations_test() {
    assert!(MIGRATIONS.validate().is_ok());
}
