//! Stores and loads protocol configurations by commitment.

use std::io;

use miden_node_db::DatabaseError;
use miden_node_db::sqlite::{ReadTx, WriteTx};
use miden_protocol::Word;
use miden_protocol::protocol_config::ProtocolConfig;
use miden_protocol::utils::serde::{ByteReader, Deserializable, Serializable, SliceReader};

const INSERT_SQL: &str = include_str!("insert.sql");
const SELECT_SQL: &str = include_str!("select.sql");

/// Loads a protocol configuration and verifies its serialized value and commitment.
pub fn load(tx: &ReadTx<'_>, commitment: Word) -> Result<Option<ProtocolConfig>, DatabaseError> {
    let bytes = tx
        .query(SELECT_SQL, &[&commitment], |row| row.get::<Vec<u8>>(0))?
        .into_iter()
        .next();
    let Some(bytes) = bytes else {
        return Ok(None);
    };

    let mut reader = SliceReader::new(&bytes);
    let config = ProtocolConfig::read_from(&mut reader)
        .map_err(|err| DatabaseError::deserialization("ProtocolConfig", err))?;
    if reader.has_more_bytes() {
        return Err(invalid_config(format!("protocol config {commitment} has trailing bytes")));
    }
    let calculated = config.to_commitment();
    if calculated != commitment {
        return Err(invalid_config(format!(
            "protocol config commitment mismatch: expected {commitment}, got {calculated}"
        )));
    }

    Ok(Some(config))
}

/// Inserts a protocol configuration if its commitment is not stored.
pub fn insert(tx: &WriteTx<'_>, config: &ProtocolConfig) -> Result<(), DatabaseError> {
    let commitment = config.to_commitment();
    tx.execute(INSERT_SQL, &[&commitment, &config.to_bytes()])?;
    Ok(())
}

fn invalid_config(message: String) -> DatabaseError {
    DatabaseError::deserialization(
        "ProtocolConfig",
        io::Error::new(io::ErrorKind::InvalidData, message),
    )
}
