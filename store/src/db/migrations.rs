use once_cell::sync::Lazy;
use rusqlite_migration::{Migrations, M};

pub static MIGRATIONS: Lazy<Migrations> = Lazy::new(|| {
    Migrations::new(vec![M::up(
        "
        CREATE TABLE
            block_headers
        (
            block_num INTEGER NOT NULL,
            block_header BLOB NOT NULL,

            PRIMARY KEY (block_num),
            CONSTRAINT block_header_block_num_is_u32 CHECK (block_num >= 0 AND block_num < 4294967296)
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE
            notes
        (
            block_num INTEGER NOT NULL,
            note_index INTEGER NOT NULL,
            note_hash BLOB NOT NULL,
            sender INTEGER NOT NULL,
            tag INTEGER NOT NULL,
            merkle_path BLOB NOT NULL,

            PRIMARY KEY (block_num, note_index),
            CONSTRAINT notes_block_number_is_u32 CHECK (block_num >= 0 AND block_num < 4294967296),
            CONSTRAINT notes_note_index_is_u32 CHECK (note_index >= 0 AND note_index < 4294967296),
            FOREIGN KEY (block_num) REFERENCES block_headers (block_num)
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE
            accounts
        (
            account_id INTEGER NOT NULL,
            account_hash BLOB NOT NULL,
            block_num INTEGER NOT NULL,

            PRIMARY KEY (account_id),
            FOREIGN KEY (block_num) REFERENCES block_headers (block_num),
            CONSTRAINT accounts_block_num_is_u32 CHECK (block_num >= 0 AND block_num < 4294967296)
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE
            nullifiers
        (
            nullifier BLOB NOT NULL,
            nullifier_prefix INTEGER NOT NULL,
            block_number INTEGER NOT NULL,

            PRIMARY KEY (nullifier),
            CONSTRAINT nullifiers_nullifier_is_digest CHECK (length(nullifier) = 32),
            CONSTRAINT nullifiers_nullifier_prefix_is_u16 CHECK (nullifier_prefix >= 0 AND nullifier_prefix < 65536),
            CONSTRAINT nullifiers_block_number_is_u32 CHECK (block_number >= 0 AND block_number < 4294967296),
            FOREIGN KEY (block_number) REFERENCES block_headers (block_num)
        ) STRICT, WITHOUT ROWID;
        ",
    )])
});

#[test]
fn migrations_test() {
    assert!(MIGRATIONS.validate().is_ok());
}
