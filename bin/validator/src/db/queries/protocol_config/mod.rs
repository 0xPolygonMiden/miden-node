//! Stores and loads protocol configurations by commitment.

use miden_node_db::DatabaseError;
use miden_node_db::sqlite::{ReadTx, WriteTx};
use miden_protocol::Word;
use miden_protocol::protocol_config::ProtocolConfig;

const INSERT_SQL: &str = include_str!("insert.sql");
const SELECT_SQL: &str = include_str!("select.sql");

/// Loads a protocol configuration and verifies its serialized value and commitment.
pub fn load(tx: &ReadTx<'_>, commitment: Word) -> Result<Option<ProtocolConfig>, DatabaseError> {
    Ok(tx
        .query(SELECT_SQL, &[&commitment], |row| row.get::<ProtocolConfig>(0))?
        .into_iter()
        .next())
}

/// Inserts a protocol configuration if its commitment is not stored.
pub fn insert(tx: &WriteTx<'_>, config: &ProtocolConfig) -> Result<(), DatabaseError> {
    let commitment = config.to_commitment();
    tx.execute(INSERT_SQL, &[&commitment, &config])?;
    Ok(())
}
