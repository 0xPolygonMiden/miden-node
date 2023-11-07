//! Wrapper functions for SQL statements.
use super::StateSyncUpdate;
use crate::types::{AccountId, BlockNumber};
use anyhow::anyhow;
use miden_crypto::{
    hash::rpo::RpoDigest,
    utils::{Deserializable, SliceReader},
};
use miden_node_proto::{
    account_id,
    block_header::BlockHeader,
    digest::Digest,
    merkle::MerklePath,
    note::Note,
    responses::{AccountHashUpdate, NullifierUpdate},
};
use prost::Message;
use rusqlite::{params, types::Value, Connection};
use std::rc::Rc;

#[cfg(test)]
pub fn add_nullifier(
    conn: &mut Connection,
    nullifier: RpoDigest,
    block_number: BlockNumber,
) -> anyhow::Result<usize> {
    use miden_crypto::StarkField;

    let mut stmt = conn.prepare(
        "INSERT INTO nullifiers (nullifier, nullifier_prefix, block_number) VALUES (?1, ?2, ?3);",
    )?;

    Ok(stmt.execute(params![
        nullifier.as_bytes(),
        u64_to_prefix(nullifier[0].as_int()),
        block_number
    ])?)
}

pub fn get_nullifiers(
    conn: &mut Connection
) -> Result<Vec<(RpoDigest, BlockNumber)>, anyhow::Error> {
    let mut stmt = conn.prepare("SELECT nullifier, block_number FROM nullifiers;")?;
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

/// Returns nullifiers created in the `(block_start, block_end]` range which also match the
/// `nullifiers` filter.
///
/// Each value of the `nullifiers` is only the 16 most significat bits of the nullifier of
/// interest to the client. This hides the details of the specific nullifier being requested.
pub fn get_nullifiers_by_block_range(
    conn: &mut Connection,
    block_start: BlockNumber,
    block_end: BlockNumber,
    nullifiers: &[u32],
) -> Result<Vec<NullifierUpdate>, anyhow::Error> {
    let nullifiers: Vec<Value> = nullifiers.iter().copied().map(u32_to_value).collect();

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
    ",
    )?;

    let mut rows = stmt.query(params![block_start, block_end, Rc::new(nullifiers)])?;

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

/// Save a [BlockHeader] to the DB.
#[cfg(test)]
pub fn add_block_header(
    conn: &mut Connection,
    block_header: BlockHeader,
) -> Result<usize, anyhow::Error> {
    let mut stmt =
        conn.prepare("INSERT INTO block_headers (block_num, block_header) VALUES (?1, ?2);")?;
    Ok(stmt.execute(params![block_header.block_num, block_header.encode_to_vec()])?)
}

/// Search for a [BlockHeader] from the DB by its `block_num`.
///
/// When `block_number` is [None], the latest block header is returned.
pub fn get_block_header(
    conn: &mut Connection,
    block_number: Option<BlockNumber>,
) -> Result<Option<BlockHeader>, anyhow::Error> {
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

/// Loads all the block headers from the DB.
pub fn get_block_headers(conn: &mut Connection) -> Result<Vec<BlockHeader>, anyhow::Error> {
    let mut stmt = conn.prepare("SELECT block_header FROM block_headers;")?;
    let mut rows = stmt.query([])?;
    let mut result = vec![];
    while let Some(row) = rows.next()? {
        let block_header_data = row.get_ref(0)?.as_blob()?;
        let block_header = BlockHeader::decode(block_header_data)?;
        result.push(block_header);
    }

    Ok(result)
}

/// Save a [Note] to the DB.
#[cfg(test)]
pub fn add_note(
    conn: &mut Connection,
    note: Note,
) -> Result<usize, anyhow::Error> {
    use miden_node_store::errors::DbError;

    let mut stmt = conn.prepare(
        "
        INSERT INTO
        notes
        (
            block_num,
            note_index,
            note_hash,
            sender,
            tag,
            num_assets,
            merkle_path
        )
        VALUES
        (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7
        );",
    )?;
    let res = stmt.execute(params![
        note.block_num,
        note.note_index,
        note.note_hash.ok_or(DbError::NoteMissingHash)?.encode_to_vec(),
        note.sender,
        note.tag,
        note.num_assets,
        note.merkle_path.ok_or(DbError::NoteMissingMerklePath)?.encode_to_vec(),
    ])?;
    Ok(res)
}

/// Return notes matching the tag and account_ids search criteria.
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
pub fn get_notes_since_block_by_tag_and_sender(
    conn: &mut Connection,
    tags: &[u32],
    account_ids: &[AccountId],
    block_num: BlockNumber,
) -> Result<Vec<Note>, anyhow::Error> {
    let tags: Vec<Value> = tags.iter().copied().map(u32_to_value).collect();
    let account_ids = account_ids
        .iter()
        .copied()
        .map(u64_to_value)
        .collect::<Result<Vec<Value>, anyhow::Error>>()?;

    let mut stmt = conn.prepare(
        "
        SELECT
            block_num,
            note_index,
            note_hash,
            sender,
            tag,
            num_assets,
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
            -- load notes that matches any of tags
            (tag >> 48) IN rarray(?1);
    ",
    )?;
    let mut rows = stmt.query(params![Rc::new(tags), Rc::new(account_ids), block_num])?;

    let mut res = Vec::new();
    while let Some(row) = rows.next()? {
        let block_num = row.get(0)?;
        let note_index = row.get(1)?;
        let note_hash_data = row.get_ref(2)?.as_blob()?;
        let note_hash = Some(decode_protobuf_digest(note_hash_data)?);
        let sender = row.get(3)?;
        let tag = row.get(4)?;
        let num_assets = row.get(5)?;
        let merkle_path_data = row.get_ref(6)?.as_blob()?;
        let merkle_path = Some(MerklePath::decode(merkle_path_data)?);

        let note = Note {
            block_num,
            note_index,
            note_hash,
            sender,
            tag,
            num_assets,
            merkle_path,
        };
        res.push(note);
    }
    Ok(res)
}

/// Inserts or updates an account's hash in the DB.
#[cfg(test)]
pub fn update_account_hash(
    conn: &mut Connection,
    account_id: AccountId,
    account_hash: Digest,
    block_num: BlockNumber,
) -> Result<usize, anyhow::Error> {
    let mut stmt = conn.prepare("INSERT OR REPLACE INTO accounts (account_id, account_hash, block_num) VALUES (?1, ?2, ?3);")?;
    Ok(stmt.execute(params![account_id, account_hash.encode_to_vec(), block_num])?)
}

// Returns the account hash of the ones that have changed in the range `(block_start, block_end]`.
pub fn get_account_hash_by_block_range(
    conn: &mut Connection,
    block_start: BlockNumber,
    block_end: BlockNumber,
    account_ids: &[AccountId],
) -> Result<Vec<AccountHashUpdate>, anyhow::Error> {
    let account_ids = account_ids
        .iter()
        .copied()
        .map(u64_to_value)
        .collect::<Result<Vec<Value>, anyhow::Error>>()?;

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
    ",
    )?;

    let mut rows = stmt.query(params![block_start, block_end, Rc::new(account_ids)])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let account_id_data: u64 = row.get(0)?;
        let account_id: account_id::AccountId = account_id_data.into();
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

/// Loads all the account hashes from the DB.
pub fn get_account_hashes(
    conn: &mut Connection
) -> Result<Vec<(AccountId, Digest)>, anyhow::Error> {
    let mut stmt = conn.prepare("SELECT account_id, account_hash FROM accounts")?;
    let mut rows = stmt.query([])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let account_id: u64 = row.get(0)?;
        let account_hash_data = row.get_ref(1)?.as_blob()?;
        let account_hash = Digest::decode(account_hash_data)?;

        result.push((account_id, account_hash));
    }

    Ok(result)
}

pub fn get_state_sync(
    conn: &mut Connection,
    block_num: BlockNumber,
    account_ids: &[AccountId],
    note_tag_prefixes: &[u32],
    nullifiers_prefix: &[u32],
) -> Result<StateSyncUpdate, anyhow::Error> {
    let notes =
        get_notes_since_block_by_tag_and_sender(conn, note_tag_prefixes, account_ids, block_num)?;

    let (block_header, chain_tip) = if !notes.is_empty() {
        let block_header = get_block_header(conn, Some(notes[0].block_num))?
            .ok_or(anyhow!("Block db is empty"))?;
        let tip = get_block_header(conn, None)?.ok_or(anyhow!("Block db is empty"))?;

        (block_header, tip.block_num)
    } else {
        let block_header = get_block_header(conn, None)?.ok_or(anyhow!("Block db is empty"))?;

        let block_num = block_header.block_num;
        (block_header, block_num)
    };

    let account_updates =
        get_account_hash_by_block_range(conn, block_num, block_header.block_num, account_ids)?;

    let nullifiers =
        get_nullifiers_by_block_range(conn, block_num, block_header.block_num, nullifiers_prefix)?;

    Ok(StateSyncUpdate {
        notes,
        block_header,
        chain_tip,
        account_updates,
        nullifiers,
    })
}

// UTILITIES
// ================================================================================================

/// Decodes a blob from the database into a [Digest].
fn decode_protobuf_digest(data: &[u8]) -> Result<Digest, anyhow::Error> {
    Ok(Digest::decode(data)?)
}

/// Decodes a blob from the database into a [RpoDigest].
fn decode_rpo_digest(data: &[u8]) -> Result<RpoDigest, anyhow::Error> {
    let mut reader = SliceReader::new(data);
    RpoDigest::read_from(&mut reader).map_err(|_| anyhow!("Decoding nullifier from DB failed"))
}

/// Returns the high bits of the `u64` value used during searches.
#[cfg(test)]
pub fn u64_to_prefix(v: u64) -> u32 {
    (v >> 48) as u32
}

/// Converts a `u64` into a [Value].
///
/// Sqlite uses `i64` as its internal representation format.
pub fn u64_to_value(v: u64) -> Result<Value, anyhow::Error> {
    let v: i64 = v.try_into()?;
    Ok(Value::Integer(v))
}

/// Converts a `u32` into a [Value].
///
/// Sqlite uses `i64` as its internal representation format.
pub fn u32_to_value(v: u32) -> Value {
    let v: i64 = v.into();
    Value::Integer(v)
}
