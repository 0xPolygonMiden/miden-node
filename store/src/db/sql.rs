//! Wrapper functions for SQL statements.
use std::rc::Rc;

use miden_crypto::{
    hash::rpo::RpoDigest,
    utils::{Deserializable, SliceReader},
};
use miden_node_proto::{
    account::{self, AccountId as AccountIdProto, AccountInfo},
    block_header::BlockHeader,
    digest::Digest,
    merkle::MerklePath,
    note::Note,
    responses::{AccountHashUpdate, NullifierUpdate},
};
use prost::Message;
use rusqlite::{params, types::Value, Connection, Transaction};

use super::{Result, StateSyncUpdate};
use crate::{
    errors::{DbError, StateError},
    types::{AccountId, BlockNumber},
};

/// Insert nullifiers to the DB using the given [Transaction].
///
/// # Returns
///
/// The number of affected rows.
///
/// # Note
///
/// The [Transaction] object is not consumed. It's up to the caller to commit or rollback the
/// transaction.
pub fn insert_nullifiers_for_block(
    transaction: &Transaction,
    nullifiers: &[RpoDigest],
    block_num: BlockNumber,
) -> Result<usize> {
    use miden_crypto::StarkField;

    let mut stmt = transaction.prepare(
        "INSERT INTO nullifiers (nullifier, nullifier_prefix, block_number) VALUES (?1, ?2, ?3);",
    )?;

    let mut count = 0;
    for nullifier in nullifiers.iter() {
        count += stmt.execute(params![
            nullifier.as_bytes(),
            u64_to_prefix(nullifier[0].as_int()),
            block_num,
        ])?
    }
    Ok(count)
}

/// Select all nullifiers from the DB using the given [Connection].
///
/// # Returns
///
/// A vector with nullifiers and the block height at which they where created, or an error.
pub fn select_nullifiers(conn: &mut Connection) -> Result<Vec<(RpoDigest, BlockNumber)>> {
    let mut stmt =
        conn.prepare("SELECT nullifier, block_number FROM nullifiers ORDER BY block_number ASC;")?;
    let mut rows = stmt.query([])?;

    let mut result = vec![];
    while let Some(row) = rows.next()? {
        let nullifier_data = row.get_ref(0)?.as_blob()?;
        let nullifier = decode_rpo_digest(nullifier_data)?;
        let block_number = row.get(1)?;
        result.push((nullifier, block_number));
    }
    Ok(result)
}

/// Select all notes from the DB using the given [Connection].
///
///
/// # Returns
///
/// A vector with notes, or an error.
pub fn select_notes(conn: &mut Connection) -> Result<Vec<Note>> {
    let mut stmt = conn.prepare("SELECT * FROM notes ORDER BY block_num ASC;")?;
    let mut rows = stmt.query([])?;

    let mut notes = vec![];
    while let Some(row) = rows.next()? {
        let note_hash_data = row.get_ref(2)?.as_blob()?;
        let note_hash = Digest::decode(note_hash_data)?;

        let merkle_path_data = row.get_ref(5)?.as_blob()?;
        let merkle_path = MerklePath::decode(merkle_path_data)?;

        notes.push(Note {
            block_num: row.get(0)?,
            note_index: row.get(1)?,
            note_hash: Some(note_hash),
            sender: column_value_as_u64(row, 3)?,
            tag: column_value_as_u64(row, 4)?,
            merkle_path: Some(merkle_path),
        })
    }
    Ok(notes)
}

/// Select all accounts from the DB using the given [Connection].
///
///
/// # Returns
///
/// A vector with accounts, or an error.
pub fn select_accounts(conn: &mut Connection) -> Result<Vec<AccountInfo>> {
    let mut stmt = conn.prepare("SELECT * FROM accounts ORDER BY block_num ASC;")?;
    let mut rows = stmt.query([])?;

    let mut accounts = vec![];
    while let Some(row) = rows.next()? {
        let account_hash_data = row.get_ref(1)?.as_blob()?;
        let account_hash = Digest::decode(account_hash_data)?;

        let account_id_data = column_value_as_u64(row, 0)?;
        let account_id = AccountIdProto::from(account_id_data);

        accounts.push(AccountInfo {
            account_id: Some(account_id),
            account_hash: Some(account_hash),
            block_num: row.get(2)?,
        })
    }
    Ok(accounts)
}

/// Select nullifiers created between `(block_start, block_end]` that also match the
/// `nullifier_prefixes` filter using the given [Connection].
///
/// Each value of the `nullifier_prefixes` is only the 16 most significant bits of the nullifier of
/// interest to the client. This hides the details of the specific nullifier being requested.
///
/// # Returns
///
/// A vector of [NullifierUpdate] with the nullifiers and the block height at which they where
/// created, or an error.
pub fn select_nullifiers_by_block_range(
    conn: &mut Connection,
    block_start: BlockNumber,
    block_end: BlockNumber,
    nullifier_prefixes: &[u32],
) -> Result<Vec<NullifierUpdate>> {
    let nullifier_prefixes: Vec<Value> =
        nullifier_prefixes.iter().copied().map(u32_to_value).collect();

    let mut stmt = conn.prepare(
        "
        SELECT
            nullifier,
            block_number
        FROM
            nullifiers
        WHERE
            block_number > ?1 AND
            block_number <= ?2 AND
            nullifier_prefix IN rarray(?3)
        ORDER BY
            block_number ASC
    ",
    )?;

    let mut rows = stmt.query(params![block_start, block_end, Rc::new(nullifier_prefixes)])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let nullifier_data = row.get_ref(0)?.as_blob()?;
        let nullifier: Digest = decode_rpo_digest(nullifier_data)?.into();
        let block_num = row.get(1)?;
        result.push(NullifierUpdate {
            nullifier: Some(nullifier),
            block_num,
        });
    }
    Ok(result)
}

/// Insert a [BlockHeader] to the DB using the given [Transaction].
///
/// # Returns
///
/// The number of affected rows.
///
/// # Note
///
/// The [Transaction] object is not consumed. It's up to the caller to commit or rollback the
/// transaction.
pub fn insert_block_header(
    transaction: &Transaction,
    block_header: &BlockHeader,
) -> Result<usize> {
    let mut stmt = transaction
        .prepare("INSERT INTO block_headers (block_num, block_header) VALUES (?1, ?2);")?;
    Ok(stmt.execute(params![block_header.block_num, block_header.encode_to_vec()])?)
}

/// Select a [BlockHeader] from the DB by its `block_num` using the given [Connection].
///
/// # Returns
///
/// When `block_number` is [None], the latest block header is returned. Otherwise the block with the
/// given block height is returned.
pub fn select_block_header_by_block_num(
    conn: &mut Connection,
    block_number: Option<BlockNumber>,
) -> Result<Option<BlockHeader>> {
    let mut stmt;
    let mut rows = match block_number {
        Some(block_number) => {
            stmt = conn.prepare("SELECT block_header FROM block_headers WHERE block_num = ?1")?;
            stmt.query([block_number])?
        },
        None => {
            stmt = conn.prepare(
                "SELECT block_header FROM block_headers ORDER BY block_num DESC LIMIT 1",
            )?;
            stmt.query([])?
        },
    };

    match rows.next()? {
        Some(row) => {
            let data = row.get_ref(0)?.as_blob()?;
            Ok(Some(BlockHeader::decode(data)?))
        },
        None => Ok(None),
    }
}

/// Select all block headers from the DB using the given [Connection].
///
/// # Returns
///
/// A vector of [BlockHeader] or an error.
pub fn select_block_headers(conn: &mut Connection) -> Result<Vec<BlockHeader>> {
    let mut stmt =
        conn.prepare("SELECT block_header FROM block_headers ORDER BY block_num ASC;")?;
    let mut rows = stmt.query([])?;
    let mut result = vec![];
    while let Some(row) = rows.next()? {
        let block_header_data = row.get_ref(0)?.as_blob()?;
        let block_header = BlockHeader::decode(block_header_data)?;
        result.push(block_header);
    }

    Ok(result)
}

/// Insert notes to the DB using the given [Transaction].
///
/// # Returns
///
/// The number of affected rows.
///
/// # Note
///
/// The [Transaction] object is not consumed. It's up to the caller to commit or rollback the
/// transaction.
pub fn insert_notes(
    transaction: &Transaction,
    notes: &[Note],
) -> Result<usize> {
    let mut stmt = transaction.prepare(
        "
        INSERT INTO
        notes
        (
            block_num,
            note_index,
            note_hash,
            sender,
            tag,
            merkle_path
        )
        VALUES
        (
            ?1, ?2, ?3, ?4, ?5, ?6 
        );",
    )?;

    let mut count = 0;
    for note in notes.iter() {
        count += stmt.execute(params![
            note.block_num,
            note.note_index,
            note.note_hash
                .clone()
                .ok_or(StateError::MissingFieldInProtobufRepresentation {
                    entity: "note",
                    field_name: "note_hash"
                })?
                .encode_to_vec(),
            u64_to_value(note.sender),
            u64_to_value(note.tag),
            note.merkle_path
                .clone()
                .ok_or(StateError::MissingFieldInProtobufRepresentation {
                    entity: "note",
                    field_name: "merkle_path"
                })?
                .encode_to_vec(),
        ])?;
    }

    Ok(count)
}

/// Select notes matching the tag and account_ids search criteria using the given [Connection].
///
/// # Returns
///
/// - Empty vector if no tag created after `block_num` match `tags` or `account_ids`.
/// - Otherwise, notes which the 16 high bits match `tags`, or the `sender` is one of the
///   `account_ids`.
///
/// # Note
///
/// This method returns notes from a single block. To fetch all notes up to the chain tip,
/// multiple requests are necessary.
pub fn select_notes_since_block_by_tag_and_sender(
    conn: &mut Connection,
    tags: &[u32],
    account_ids: &[AccountId],
    block_num: BlockNumber,
) -> Result<Vec<Note>> {
    let tags: Vec<Value> = tags.iter().copied().map(u32_to_value).collect();
    let account_ids: Vec<Value> = account_ids.iter().copied().map(u64_to_value).collect();

    let mut stmt = conn.prepare(
        "
        SELECT
            block_num,
            note_index,
            note_hash,
            sender,
            tag,
            merkle_path
        FROM
            notes
        WHERE
            -- find the next block which contains at least one note with a matching tag
            block_num = (
                SELECT
                    block_num
                FROM
                    notes
                WHERE
                    ((tag >> 48) IN rarray(?1) OR sender IN rarray(?2)) AND
                    block_num > ?3
                ORDER BY
                    block_num ASC
                LIMIT
                    1
            ) AND
            -- filter the block's notes and return only the ones matching the requested tags
            ((tag >> 48) IN rarray(?1) OR sender IN rarray(?2));
    ",
    )?;
    let mut rows = stmt.query(params![Rc::new(tags), Rc::new(account_ids), block_num])?;

    let mut res = Vec::new();
    while let Some(row) = rows.next()? {
        let block_num = row.get(0)?;
        let note_index = row.get(1)?;
        let note_hash_data = row.get_ref(2)?.as_blob()?;
        let note_hash = Some(decode_protobuf_digest(note_hash_data)?);
        let sender = column_value_as_u64(row, 3)?;
        let tag = column_value_as_u64(row, 4)?;
        let merkle_path_data = row.get_ref(5)?.as_blob()?;
        let merkle_path = Some(MerklePath::decode(merkle_path_data)?);

        let note = Note {
            block_num,
            note_index,
            note_hash,
            sender,
            tag,
            merkle_path,
        };
        res.push(note);
    }
    Ok(res)
}

/// Inserts or updates accounts to the DB using the given [Transaction].
///
/// # Returns
///
/// The number of affected rows.
///
/// # Note
///
/// The [Transaction] object is not consumed. It's up to the caller to commit or rollback the
/// transaction.
pub fn upsert_accounts_with_blocknum(
    transaction: &Transaction,
    accounts: &[(AccountId, Digest)],
    block_num: BlockNumber,
) -> Result<usize> {
    let mut stmt = transaction.prepare("INSERT OR REPLACE INTO accounts (account_id, account_hash, block_num) VALUES (?1, ?2, ?3);")?;

    let mut count = 0;
    for (account_id, account_hash) in accounts.iter() {
        count += stmt.execute(params![
            u64_to_value(*account_id),
            account_hash.encode_to_vec(),
            block_num
        ])?
    }
    Ok(count)
}

/// Select [AccountHashUpdate] from the DB using the given [Connection], given that the account
/// update was done between `(block_start, block_end]`.
///
/// # Returns
///
/// The vector of [AccountHashUpdate] with the matching accounts.
pub fn select_accounts_by_block_range(
    conn: &mut Connection,
    block_start: BlockNumber,
    block_end: BlockNumber,
    account_ids: &[AccountId],
) -> Result<Vec<AccountHashUpdate>> {
    let account_ids: Vec<Value> = account_ids.iter().copied().map(u64_to_value).collect();

    let mut stmt = conn.prepare(
        "
        SELECT
            account_id, account_hash, block_num
        FROM
            accounts
        WHERE
            block_num > ?1 AND
            block_num <= ?2 AND
            account_id IN rarray(?3)
        ORDER BY
            block_num ASC
    ",
    )?;

    let mut rows = stmt.query(params![block_start, block_end, Rc::new(account_ids)])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let account_id_data = column_value_as_u64(row, 0)?;
        let account_id: account::AccountId = account_id_data.into();
        let account_hash_data = row.get_ref(1)?.as_blob()?;
        let account_hash = Digest::decode(account_hash_data)?;
        let block_num = row.get(2)?;

        result.push(AccountHashUpdate {
            account_id: Some(account_id),
            account_hash: Some(account_hash),
            block_num,
        });
    }

    Ok(result)
}

/// Select all account hashes from the DB using the given [Connection].
///
/// # Returns
///
/// The vector with the account id and corresponding hash, or an error.
pub fn select_account_hashes(conn: &mut Connection) -> Result<Vec<(AccountId, Digest)>> {
    let mut stmt =
        conn.prepare("SELECT account_id, account_hash FROM accounts ORDER BY block_num ASC;")?;
    let mut rows = stmt.query([])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let account_id = column_value_as_u64(row, 0)?;
        let account_hash_data = row.get_ref(1)?.as_blob()?;
        let account_hash = Digest::decode(account_hash_data)?;

        result.push((account_id, account_hash));
    }

    Ok(result)
}

/// Loads the state necessary for a state sync.
pub fn get_state_sync(
    conn: &mut Connection,
    block_num: BlockNumber,
    account_ids: &[AccountId],
    note_tag_prefixes: &[u32],
    nullifier_prefixes: &[u32],
) -> Result<StateSyncUpdate> {
    let notes = select_notes_since_block_by_tag_and_sender(
        conn,
        note_tag_prefixes,
        account_ids,
        block_num,
    )?;

    let (block_header, chain_tip) = if !notes.is_empty() {
        let block_header = select_block_header_by_block_num(conn, Some(notes[0].block_num))?
            .ok_or(DbError::BlockDbIsEmpty)?;
        let tip = select_block_header_by_block_num(conn, None)?.ok_or(DbError::BlockDbIsEmpty)?;

        (block_header, tip.block_num)
    } else {
        let block_header =
            select_block_header_by_block_num(conn, None)?.ok_or(DbError::BlockDbIsEmpty)?;

        let block_num = block_header.block_num;
        (block_header, block_num)
    };

    let account_updates =
        select_accounts_by_block_range(conn, block_num, block_header.block_num, account_ids)?;

    let nullifiers = select_nullifiers_by_block_range(
        conn,
        block_num,
        block_header.block_num,
        nullifier_prefixes,
    )?;

    Ok(StateSyncUpdate {
        notes,
        block_header,
        chain_tip,
        account_updates,
        nullifiers,
    })
}

/// Updates the DB with the state of a new block.
///
///
/// # Returns
///
/// The number of affected rows in the DB.
pub fn apply_block(
    transaction: &Transaction,
    block_header: &BlockHeader,
    notes: &[Note],
    nullifiers: &[RpoDigest],
    accounts: &[(AccountId, Digest)],
) -> Result<usize> {
    let mut count = 0;
    count += insert_block_header(transaction, block_header)?;
    count += insert_notes(transaction, notes)?;
    count += upsert_accounts_with_blocknum(transaction, accounts, block_header.block_num)?;
    count += insert_nullifiers_for_block(transaction, nullifiers, block_header.block_num)?;
    Ok(count)
}

// UTILITIES
// ================================================================================================

/// Decodes a blob from the database into a [Digest].
fn decode_protobuf_digest(data: &[u8]) -> Result<Digest> {
    Ok(Digest::decode(data)?)
}

/// Decodes a blob from the database into a [RpoDigest].
fn decode_rpo_digest(data: &[u8]) -> Result<RpoDigest> {
    let mut reader = SliceReader::new(data);
    RpoDigest::read_from(&mut reader).map_err(DbError::NullifierDecodingError)
}

/// Returns the high bits of the `u64` value used during searches.
pub(crate) fn u64_to_prefix(v: u64) -> u32 {
    (v >> 48) as u32
}

/// Converts a `u64` into a [Value].
///
/// Sqlite uses `i64` as its internal representation format. Note that the `as` operator performs a
/// lossless conversion from `u64` to `i64`.
fn u64_to_value(v: u64) -> Value {
    Value::Integer(v as i64)
}

/// Converts a `u32` into a [Value].
///
/// Sqlite uses `i64` as its internal representation format.
fn u32_to_value(v: u32) -> Value {
    let v: i64 = v.into();
    Value::Integer(v)
}

/// Gets a `u64`` value from the database.
///
/// Sqlite uses `i64` as its internal representation format, and so when retrieving
/// we need to make sure we cast as `u64` to get the original value
fn column_value_as_u64<I: rusqlite::RowIndex>(
    row: &rusqlite::Row<'_>,
    index: I,
) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    Ok(value as u64)
}
