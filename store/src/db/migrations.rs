use once_cell::sync::Lazy;
use rusqlite_migration::{Migrations, M};

pub static MIGRATIONS: Lazy<Migrations> =
    Lazy::new(|| Migrations::new(vec![M::up(include_str!("migrations/001-init.sql"))]));

#[test]
fn migrations_validate() {
    assert_eq!(MIGRATIONS.validate(), Ok(()));
}
