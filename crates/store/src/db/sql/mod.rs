//! Wrapper functions for SQL statements.

#[macro_use]
pub(crate) mod utils;

use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    num::NonZeroUsize,
    rc::Rc,
};

use miden_node_proto::domain::account::{AccountInfo, AccountSummary};
use miden_objects::{
    Digest, Word,
    account::{
        Account, AccountDelta, AccountId, AccountStorageDelta, AccountVaultDelta,
        FungibleAssetDelta, NonFungibleAssetDelta, NonFungibleDeltaAction, StorageMapDelta,
        delta::AccountUpdateDetails,
    },
    asset::NonFungibleAsset,
    block::{BlockAccountUpdate, BlockHeader, BlockNoteIndex, BlockNumber},
    crypto::{hash::rpo::RpoDigest, merkle::MerklePath},
    note::{NoteExecutionMode, NoteId, NoteInclusionProof, NoteMetadata, NoteType, Nullifier},
    transaction::TransactionId,
    utils::serde::{Deserializable, Serializable},
};
use rusqlite::{Connection, Transaction, params, types::Value};
use utils::{read_block_number, read_from_blob_column};

use super::{
    NoteRecord, NoteSyncRecord, NoteSyncUpdate, NullifierInfo, Result, StateSyncUpdate,
    TransactionSummary,
};
use crate::{
    db::sql::utils::{
        account_info_from_row, account_summary_from_row, column_value_as_u64, get_nullifier_prefix,
        read_from_blob_or_null_column, u64_to_value,
    },
    errors::{DatabaseError, NoteSyncError, StateSyncError},
};
// ACCOUNT QUERIES
// ================================================================================================

/// Select all accounts from the DB using the given [Connection].
///
/// # Returns
///
/// A vector with accounts, or an error.
#[cfg(test)]
pub fn select_all_accounts(conn: &mut Connection) -> Result<Vec<AccountInfo>> {
    let mut stmt = conn.prepare_cached(
        "
        SELECT
            account_id,
            account_hash,
            block_num,
            details
        FROM
            accounts
        ORDER BY
            block_num ASC;
    ",
    )?;
    let mut rows = stmt.query([])?;

    let mut accounts = vec![];
    while let Some(row) = rows.next()? {
        accounts.push(account_info_from_row(row)?);
    }
    Ok(accounts)
}

/// Select all account hashes from the DB using the given [Connection].
///
/// # Returns
///
/// The vector with the account id and corresponding hash, or an error.
pub fn select_all_account_hashes(conn: &mut Connection) -> Result<Vec<(AccountId, RpoDigest)>> {
    let mut stmt = conn
        .prepare_cached("SELECT account_id, account_hash FROM accounts ORDER BY block_num ASC;")?;
    let mut rows = stmt.query([])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let account_id: AccountId = read_from_blob_column(row, 0)?;
        let account_hash_data = row.get_ref(1)?.as_blob()?;
        let account_hash = RpoDigest::read_from_bytes(account_hash_data)?;

        result.push((account_id, account_hash));
    }

    Ok(result)
}

/// Select [`AccountSummary`] from the DB using the given [Connection], given that the account
/// update was done between `(block_start, block_end]`.
///
/// # Returns
///
/// The vector of [`AccountSummary`] with the matching accounts.
pub fn select_accounts_by_block_range(
    conn: &mut Connection,
    block_start: BlockNumber,
    block_end: BlockNumber,
    account_ids: &[AccountId],
) -> Result<Vec<AccountSummary>> {
    let mut stmt = conn.prepare_cached(
        "
        SELECT
            account_id,
            account_hash,
            block_num
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

    let account_ids: Vec<Value> = account_ids
        .iter()
        .copied()
        .map(|account_id| account_id.to_bytes().into())
        .collect();
    let mut rows =
        stmt.query(params![block_start.as_u32(), block_end.as_u32(), Rc::new(account_ids)])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(account_summary_from_row(row)?);
    }

    Ok(result)
}

/// Select the latest account details by account id from the DB using the given [Connection].
///
/// # Returns
///
/// The latest account details, or an error.
pub fn select_account(conn: &mut Connection, account_id: AccountId) -> Result<AccountInfo> {
    let mut stmt = conn.prepare_cached(
        "
        SELECT
            account_id,
            account_hash,
            block_num,
            details
        FROM
            accounts
        WHERE
            account_id = ?1;
    ",
    )?;

    let mut rows = stmt.query(params![account_id.to_bytes()])?;
    let row = rows.next()?.ok_or(DatabaseError::AccountNotFoundInDb(account_id))?;

    account_info_from_row(row)
}

/// Select the latest accounts' details filtered by IDs from the DB using the given [Connection].
///
/// # Returns
///
/// The account details vector, or an error.
pub fn select_accounts_by_ids(
    conn: &mut Connection,
    account_ids: &[AccountId],
) -> Result<Vec<AccountInfo>> {
    let mut stmt = conn.prepare_cached(
        "
        SELECT
            account_id,
            account_hash,
            block_num,
            details
        FROM
            accounts
        WHERE
            account_id IN rarray(?1);
    ",
    )?;

    let account_ids: Vec<Value> = account_ids
        .iter()
        .copied()
        .map(|account_id| account_id.to_bytes().into())
        .collect();
    let mut rows = stmt.query(params![Rc::new(account_ids)])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(account_info_from_row(row)?);
    }

    Ok(result)
}

/// Selects and merges account deltas by account id and block range from the DB using the given
/// [Connection].
///
/// # Note:
///
/// `block_start` is exclusive and `block_end` is inclusive.
///
/// # Returns
///
/// The resulting account delta, or an error.
#[allow(clippy::too_many_lines, reason = "mostly just formatted sql text")]
pub fn select_account_delta(
    conn: &mut Connection,
    account_id: AccountId,
    block_start: BlockNumber,
    block_end: BlockNumber,
) -> Result<Option<AccountDelta>> {
    let mut select_nonce_stmt = conn.prepare_cached(
        "
        SELECT
            nonce
        FROM
            account_deltas
        WHERE
            account_id = ?1 AND block_num > ?2 AND block_num <= ?3
        ORDER BY
            block_num DESC
        LIMIT 1
    ",
    )?;

    let mut select_slot_updates_stmt = conn.prepare_cached(
        "
            SELECT
                slot, value
            FROM
                account_storage_slot_updates AS a
            WHERE
                account_id = ?1 AND
                block_num > ?2 AND
                block_num <= ?3 AND
                NOT EXISTS(
                    SELECT 1
                    FROM account_storage_slot_updates AS b
                    WHERE
                        b.account_id = ?1 AND
                        a.slot = b.slot AND
                        a.block_num < b.block_num AND
                        b.block_num <= ?3
                )
        ",
    )?;

    let mut select_storage_map_updates_stmt = conn.prepare_cached(
        "
        SELECT
            slot, key, value
        FROM
            account_storage_map_updates AS a
        WHERE
            account_id = ?1 AND
            block_num > ?2 AND
            block_num <= ?3 AND
            NOT EXISTS(
                SELECT 1
                FROM account_storage_map_updates AS b
                WHERE
                    b.account_id = ?1 AND
                    a.slot = b.slot AND
                    a.key = b.key AND
                    a.block_num < b.block_num AND
                    b.block_num <= ?3
            )
        ",
    )?;

    let mut select_fungible_asset_deltas_stmt = conn.prepare_cached(
        "
        SELECT
            faucet_id, SUM(delta)
        FROM
            account_fungible_asset_deltas
        WHERE
            account_id = ?1 AND
            block_num > ?2 AND
            block_num <= ?3
        GROUP BY
            faucet_id
        ",
    )?;

    let mut select_non_fungible_asset_updates_stmt = conn.prepare_cached(
        "
        SELECT
            block_num, vault_key, is_remove
        FROM
            account_non_fungible_asset_updates
        WHERE
            account_id = ?1 AND
            block_num > ?2 AND
            block_num <= ?3
        ORDER BY
            block_num
        ",
    )?;

    let account_id = account_id.to_bytes();
    let nonce = match select_nonce_stmt
        .query_row(params![account_id, block_start.as_u32(), block_end.as_u32()], |row| {
            row.get::<_, u64>(0)
        }) {
        Ok(nonce) => nonce.try_into().map_err(DatabaseError::InvalidFelt)?,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let mut storage_scalars = BTreeMap::new();
    let mut rows = select_slot_updates_stmt.query(params![
        account_id,
        block_start.as_u32(),
        block_end.as_u32()
    ])?;
    while let Some(row) = rows.next()? {
        let slot = row.get(0)?;
        let value_data = row.get_ref(1)?.as_blob()?;
        let value = Word::read_from_bytes(value_data)?;
        storage_scalars.insert(slot, value);
    }

    let mut storage_maps = BTreeMap::new();
    let mut rows = select_storage_map_updates_stmt.query(params![
        account_id,
        block_start.as_u32(),
        block_end.as_u32()
    ])?;
    while let Some(row) = rows.next()? {
        let slot = row.get(0)?;
        let key_data = row.get_ref(1)?.as_blob()?;
        let key = Digest::read_from_bytes(key_data)?;
        let value_data = row.get_ref(2)?.as_blob()?;
        let value = Word::read_from_bytes(value_data)?;

        match storage_maps.entry(slot) {
            Entry::Vacant(entry) => {
                entry.insert(StorageMapDelta::new(BTreeMap::from([(key, value)])));
            },
            Entry::Occupied(mut entry) => {
                entry.get_mut().insert(key, value);
            },
        }
    }

    let mut fungible = BTreeMap::new();
    let mut rows = select_fungible_asset_deltas_stmt.query(params![
        account_id,
        block_start.as_u32(),
        block_end.as_u32()
    ])?;
    while let Some(row) = rows.next()? {
        let faucet_id: AccountId = read_from_blob_column(row, 0)?;
        let value = row.get(1)?;
        fungible.insert(faucet_id, value);
    }

    let mut non_fungible_delta = NonFungibleAssetDelta::default();
    let mut rows = select_non_fungible_asset_updates_stmt.query(params![
        account_id,
        block_start.as_u32(),
        block_end.as_u32()
    ])?;
    while let Some(row) = rows.next()? {
        let vault_key_data = row.get_ref(1)?.as_blob()?;
        let asset = NonFungibleAsset::read_from_bytes(vault_key_data)
            .map_err(|err| DatabaseError::DataCorrupted(err.to_string()))?;
        let action: usize = row.get(2)?;
        match action {
            0 => non_fungible_delta.add(asset)?,
            1 => non_fungible_delta.remove(asset)?,
            _ => {
                return Err(DatabaseError::DataCorrupted(format!(
                    "Invalid non-fungible asset delta action: {action}"
                )));
            },
        }
    }

    let storage = AccountStorageDelta::new(storage_scalars, storage_maps)?;
    let vault = AccountVaultDelta::new(FungibleAssetDelta::new(fungible)?, non_fungible_delta);

    Ok(Some(AccountDelta::new(storage, vault, Some(nonce))?))
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
pub fn upsert_accounts(
    transaction: &Transaction,
    accounts: &[BlockAccountUpdate],
    block_num: BlockNumber,
) -> Result<usize> {
    let ids: Rc<Vec<Value>> =
        Rc::new(accounts.iter().map(|update| update.account_id().to_bytes().into()).collect());
    let mut select_accounts_stmt = transaction.prepare_cached(
        "SELECT account_id, account_hash, details FROM accounts WHERE account_id IN rarray(?)",
    )?;
    let mut prev_account_data: BTreeMap<AccountId, (RpoDigest, Option<Account>)> = BTreeMap::new();
    let mut rows = select_accounts_stmt.query(params![ids])?;
    while let Some(row) = rows.next()? {
        let id: AccountId = read_from_blob_column(row, 0)?;
        let hash: RpoDigest = read_from_blob_column(row, 1)?;
        let details: Option<Account> = read_from_blob_or_null_column(row, 2)?;

        prev_account_data.insert(id, (hash, details));
    }

    let mut insert_stmt = transaction.prepare_cached(insert_sql!(accounts {
        account_id,
        account_hash,
        block_num,
        details
    }))?;
    let mut update_stmt = transaction.prepare_cached(
        "UPDATE accounts SET account_hash = ?2, block_num = ?3, details = ?4 WHERE account_id = ?1",
    )?;

    for update in accounts {
        let account_id = update.account_id();
        match (update.details(), prev_account_data.remove(&account_id)) {
            (AccountUpdateDetails::Private, Some((old_hash, details))) => {
                if old_hash == update.final_state_commitment() {
                    return Err(DatabaseError::AccountAlreadyExistsInDb(account_id));
                }
                if details.is_some() {
                    return Err(DatabaseError::AccountIsPublic(account_id));
                }
                update_stmt.execute(params![
                    account_id.to_bytes(),
                    update.final_state_commitment().to_bytes(),
                    block_num.as_u32(),
                    Option::<Vec<u8>>::None,
                ])?;
            },

            (AccountUpdateDetails::Private, None) => {
                insert_stmt.execute(params![
                    account_id.to_bytes(),
                    update.final_state_commitment().to_bytes(),
                    block_num.as_u32(),
                    Option::<Vec<u8>>::None,
                ])?;
            },

            (AccountUpdateDetails::New(account), None) => {
                debug_assert_eq!(account_id, account.id());

                if account.hash() != update.final_state_commitment() {
                    return Err(DatabaseError::AccountHashesMismatch {
                        calculated: account.hash(),
                        expected: update.final_state_commitment(),
                    });
                }

                insert_stmt.execute(params![
                    account_id.to_bytes(),
                    update.final_state_commitment().to_bytes(),
                    block_num.as_u32(),
                    Some(account.to_bytes()),
                ])?;

                let insert_delta = AccountDelta::from(account.clone());
                insert_account_delta(transaction, account_id, block_num, &insert_delta)?;
            },

            (AccountUpdateDetails::New(_), Some(_)) => {
                return Err(DatabaseError::AccountAlreadyExistsInDb(account_id));
            },

            (AccountUpdateDetails::Delta(_), None) => {
                return Err(DatabaseError::AccountNotFoundInDb(account_id));
            },

            (AccountUpdateDetails::Delta(delta), Some((old_hash, details))) => {
                if old_hash == update.final_state_commitment() {
                    return Err(DatabaseError::AccountAlreadyExistsInDb(account_id));
                }
                let Some(mut account) = details else {
                    return Err(DatabaseError::AccountNotPublic(account_id));
                };

                account.apply_delta(delta)?;

                if account.hash() != update.final_state_commitment() {
                    return Err(DatabaseError::AccountHashesMismatch {
                        calculated: account.hash(),
                        expected: update.final_state_commitment(),
                    });
                }

                update_stmt.execute(params![
                    account_id.to_bytes(),
                    update.final_state_commitment().to_bytes(),
                    block_num.as_u32(),
                    Some(account.to_bytes()),
                ])?;

                insert_account_delta(transaction, account_id, block_num, delta)?;
            },
        };
    }

    Ok(accounts.len())
}

/// Inserts account delta to the DB using the given [Transaction].
fn insert_account_delta(
    transaction: &Transaction,
    account_id: AccountId,
    block_number: BlockNumber,
    delta: &AccountDelta,
) -> Result<()> {
    let mut insert_acc_delta_stmt =
        transaction.prepare_cached(insert_sql!(account_deltas { account_id, block_num, nonce }))?;

    let mut insert_slot_update_stmt =
        transaction.prepare_cached(insert_sql!(account_storage_slot_updates {
            account_id,
            block_num,
            slot,
            value,
        }))?;

    let mut insert_storage_map_update_stmt =
        transaction.prepare_cached(insert_sql!(account_storage_map_updates {
            account_id,
            block_num,
            slot,
            key,
            value,
        }))?;

    let mut insert_fungible_asset_delta_stmt =
        transaction.prepare_cached(insert_sql!(account_fungible_asset_deltas {
            account_id,
            block_num,
            faucet_id,
            delta,
        }))?;

    let mut insert_non_fungible_asset_update_stmt =
        transaction.prepare_cached(insert_sql!(account_non_fungible_asset_updates {
            account_id,
            block_num,
            vault_key,
            is_remove,
        }))?;

    insert_acc_delta_stmt.execute(params![
        account_id.to_bytes(),
        block_number.as_u32(),
        delta.nonce().map(Into::<u64>::into).unwrap_or_default()
    ])?;

    for (&slot, value) in delta.storage().values() {
        insert_slot_update_stmt.execute(params![
            account_id.to_bytes(),
            block_number.as_u32(),
            slot,
            value.to_bytes()
        ])?;
    }

    for (&slot, map_delta) in delta.storage().maps() {
        for (key, value) in map_delta.leaves() {
            insert_storage_map_update_stmt.execute(params![
                account_id.to_bytes(),
                block_number.as_u32(),
                slot,
                key.to_bytes(),
                value.to_bytes(),
            ])?;
        }
    }

    for (&faucet_id, &delta) in delta.vault().fungible().iter() {
        insert_fungible_asset_delta_stmt.execute(params![
            account_id.to_bytes(),
            block_number.as_u32(),
            faucet_id.to_bytes(),
            delta,
        ])?;
    }

    for (&asset, action) in delta.vault().non_fungible().iter() {
        let is_remove = match action {
            NonFungibleDeltaAction::Add => 0,
            NonFungibleDeltaAction::Remove => 1,
        };
        insert_non_fungible_asset_update_stmt.execute(params![
            account_id.to_bytes(),
            block_number.as_u32(),
            asset.to_bytes(),
            is_remove,
        ])?;
    }

    Ok(())
}

// NULLIFIER QUERIES
// ================================================================================================

/// Commit nullifiers to the DB using the given [Transaction]. This inserts the nullifiers into the
/// nullifiers table, and marks the note as consumed (if it was public).
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
    nullifiers: &[Nullifier],
    block_num: BlockNumber,
) -> Result<usize> {
    let serialized_nullifiers: Vec<Value> =
        nullifiers.iter().map(Nullifier::to_bytes).map(Into::into).collect();
    let serialized_nullifiers = Rc::new(serialized_nullifiers);

    let mut stmt = transaction
        .prepare_cached("UPDATE notes SET consumed = TRUE WHERE nullifier IN rarray(?1)")?;
    let mut count = stmt.execute(params![serialized_nullifiers])?;

    let mut stmt = transaction.prepare_cached(insert_sql!(nullifiers {
        nullifier,
        nullifier_prefix,
        block_num
    }))?;

    for (nullifier, bytes) in nullifiers.iter().zip(serialized_nullifiers.iter()) {
        count +=
            stmt.execute(params![bytes, get_nullifier_prefix(nullifier), block_num.as_u32()])?;
    }
    Ok(count)
}

/// Select all nullifiers from the DB using the given [Connection].
///
/// # Returns
///
/// A vector with nullifiers and the block height at which they were created, or an error.
pub fn select_all_nullifiers(conn: &mut Connection) -> Result<Vec<(Nullifier, BlockNumber)>> {
    let mut stmt =
        conn.prepare_cached("SELECT nullifier, block_num FROM nullifiers ORDER BY block_num ASC;")?;
    let mut rows = stmt.query([])?;

    let mut result = vec![];
    while let Some(row) = rows.next()? {
        let nullifier_data = row.get_ref(0)?.as_blob()?;
        let nullifier = Nullifier::read_from_bytes(nullifier_data)?;
        let block_number = read_block_number(row, 1)?;
        result.push((nullifier, block_number));
    }
    Ok(result)
}

/// Returns nullifiers filtered by prefix and block creation height.
///
/// Each value of the `nullifier_prefixes` is only the `prefix_len` most significant bits
/// of the nullifier of interest to the client. This hides the details of the specific
/// nullifier being requested. Currently the only supported prefix length is 16 bits.
///
/// # Returns
///
/// A vector of [`NullifierInfo`] with the nullifiers and the block height at which they were
/// created, or an error.
pub fn select_nullifiers_by_prefix(
    conn: &mut Connection,
    prefix_len: u32,
    nullifier_prefixes: &[u32],
    block_num: BlockNumber,
) -> Result<Vec<NullifierInfo>> {
    assert_eq!(prefix_len, 16, "Only 16-bit prefixes are supported");

    let nullifier_prefixes: Vec<Value> =
        nullifier_prefixes.iter().copied().map(Into::into).collect();

    let mut stmt = conn.prepare_cached(
        "
        SELECT
            nullifier,
            block_num
        FROM
            nullifiers
        WHERE
            nullifier_prefix IN rarray(?1) AND
            block_num >= ?2
        ORDER BY
            block_num ASC
    ",
    )?;
    let mut rows = stmt.query(params![Rc::new(nullifier_prefixes), block_num.as_u32()])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let nullifier_data = row.get_ref(0)?.as_blob()?;
        let nullifier = Nullifier::read_from_bytes(nullifier_data)?;
        let block_num = read_block_number(row, 1)?;
        result.push(NullifierInfo { nullifier, block_num });
    }
    Ok(result)
}

// NOTE QUERIES
// ================================================================================================

/// Select all notes from the DB using the given [Connection].
///
///
/// # Returns
///
/// A vector with notes, or an error.
#[cfg(test)]
pub fn select_all_notes(conn: &mut Connection) -> Result<Vec<NoteRecord>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {} FROM notes ORDER BY block_num ASC",
        NoteRecord::SELECT_COLUMNS,
    ))?;
    let mut rows = stmt.query([])?;

    let mut notes = vec![];
    while let Some(row) = rows.next()? {
        notes.push(NoteRecord::from_row(row)?);
    }
    Ok(notes)
}

/// Insert notes to the DB using the given [Transaction]. Public notes should also have a nullifier.
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
    notes: &[(NoteRecord, Option<Nullifier>)],
) -> Result<usize> {
    let mut stmt = transaction.prepare_cached(insert_sql!(notes {
        block_num,
        batch_index,
        note_index,
        note_id,
        note_type,
        sender,
        tag,
        execution_mode,
        aux,
        execution_hint,
        merkle_path,
        consumed,
        details,
        nullifier,
    }))?;

    let mut count = 0;
    for (note, nullifier) in notes {
        let details = note.details.as_ref().map(miden_objects::utils::Serializable::to_bytes);
        count += stmt.execute(params![
            note.block_num.as_u32(),
            note.note_index.batch_idx(),
            note.note_index.note_idx_in_batch(),
            note.note_id.to_bytes(),
            note.metadata.note_type() as u8,
            note.metadata.sender().to_bytes(),
            note.metadata.tag().inner(),
            note.metadata.tag().execution_mode() as u8,
            u64_to_value(note.metadata.aux().into()),
            u64_to_value(note.metadata.execution_hint().into()),
            note.merkle_path.to_bytes(),
            // New notes are always uncomsumed.
            false,
            details,
            // Beware: `Option<T>` also implements `to_bytes`, but this is not what you want.
            nullifier.as_ref().map(Nullifier::to_bytes),
        ])?;
    }

    Ok(count)
}

/// Select notes matching the tags and account IDs search criteria using the given [Connection].
///
/// # Returns
///
/// All matching notes from the first block greater than `block_num` containing a matching note.
/// A note is considered a match if it has any of the given tags, or if its sender is one of the
/// given account IDs. If no matching notes are found at all, then an empty vector is returned.
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
) -> Result<Vec<NoteSyncRecord>> {
    let mut stmt = conn
        .prepare_cached(include_str!("queries/select_notes_since_block_by_tag_and_sender.sql"))?;

    let tags: Vec<Value> = tags.iter().copied().map(Into::into).collect();
    let account_ids: Vec<Value> = account_ids
        .iter()
        .copied()
        .map(|account_id| account_id.to_bytes().into())
        .collect();
    let mut rows = stmt.query(params![Rc::new(tags), Rc::new(account_ids), block_num.as_u32()])?;

    let mut res = Vec::new();
    while let Some(row) = rows.next()? {
        let block_num = read_block_number(row, 0)?;
        let batch_idx = row.get(1)?;
        let note_idx_in_batch = row.get(2)?;
        // SAFETY: We can assume the batch and note indices stored in the DB are valid so this
        // should never panic.
        let note_index = BlockNoteIndex::new(batch_idx, note_idx_in_batch)
            .expect("batch and note index from DB should be valid");
        let note_id = read_from_blob_column(row, 3)?;
        let note_type = row.get::<_, u8>(4)?;
        let sender = read_from_blob_column(row, 5)?;
        let tag: u32 = row.get(6)?;
        let aux: u64 = row.get(7)?;
        let aux = aux.try_into().map_err(DatabaseError::InvalidFelt)?;
        let execution_hint = column_value_as_u64(row, 8)?;
        let merkle_path = read_from_blob_column(row, 9)?;

        let metadata = NoteMetadata::new(
            sender,
            NoteType::try_from(note_type)?,
            tag.into(),
            execution_hint.try_into()?,
            aux,
        )?;

        let note = NoteSyncRecord {
            block_num,
            note_index,
            note_id,
            metadata,
            merkle_path,
        };
        res.push(note);
    }
    Ok(res)
}

/// Select Note's matching the `NoteId` using the given [Connection].
///
/// # Returns
///
/// - Empty vector if no matching `note`.
/// - Otherwise, notes which `note_id` matches the `NoteId` as bytes.
pub fn select_notes_by_id(conn: &mut Connection, note_ids: &[NoteId]) -> Result<Vec<NoteRecord>> {
    let note_ids: Vec<Value> = note_ids.iter().map(|id| id.to_bytes().into()).collect();

    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {} FROM notes WHERE note_id IN rarray(?1)",
        NoteRecord::SELECT_COLUMNS
    ))?;
    let mut rows = stmt.query(params![Rc::new(note_ids)])?;

    let mut notes = Vec::new();
    while let Some(row) = rows.next()? {
        notes.push(NoteRecord::from_row(row)?);
    }

    Ok(notes)
}

/// Select note inclusion proofs matching the `NoteId`, using the given [Connection].
///
/// # Returns
///
/// - Empty map if no matching `note`.
/// - Otherwise, note inclusion proofs, which `note_id` matches the `NoteId` as bytes.
pub fn select_note_inclusion_proofs(
    conn: &mut Connection,
    note_ids: BTreeSet<NoteId>,
) -> Result<BTreeMap<NoteId, NoteInclusionProof>> {
    let note_ids: Vec<Value> = note_ids.into_iter().map(|id| id.to_bytes().into()).collect();

    let mut select_notes_stmt = conn.prepare_cached(
        "
        SELECT
            block_num,
            note_id,
            batch_index,
            note_index,
            merkle_path
        FROM
            notes
        WHERE
            note_id IN rarray(?1)
        ORDER BY
            block_num ASC
        ",
    )?;

    let mut result = BTreeMap::new();
    let mut rows = select_notes_stmt.query(params![Rc::new(note_ids)])?;
    while let Some(row) = rows.next()? {
        let block_num: u32 = row.get(0)?;

        let note_id_data = row.get_ref(1)?.as_blob()?;
        let note_id = NoteId::read_from_bytes(note_id_data)?;

        let batch_index = row.get(2)?;
        let note_index = row.get(3)?;
        // SAFETY: We can assume the batch and note indices stored in the DB are valid so this
        // should never panic.
        let node_index_in_block = BlockNoteIndex::new(batch_index, note_index)
            .expect("batch and note index from DB should be valid")
            .leaf_index_value();

        let merkle_path_data = row.get_ref(4)?.as_blob()?;
        let merkle_path = MerklePath::read_from_bytes(merkle_path_data)?;

        let proof = NoteInclusionProof::new(block_num.into(), node_index_in_block, merkle_path)?;

        result.insert(note_id, proof);
    }

    Ok(result)
}

/// Returns a paginated batch of network notes that have not yet been consumed.
///
/// # Returns
///
/// A set of unconsumed network notes with maximum length of `limit` and a pagination token to get
/// the next set.
#[cfg_attr(not(test), expect(dead_code, reason = "gRPC method is not yet implemented"))]
pub fn unconsumed_network_notes(
    transaction: &Transaction,
    mut token: PaginationToken,
    limit: NonZeroUsize,
) -> Result<(Vec<NoteRecord>, PaginationToken)> {
    assert_eq!(
        NoteExecutionMode::Network as u8,
        0,
        "Hardcoded execution value must match query"
    );

    // Select the rowid column so that we can return a pagination token.
    //
    // rowid column _must_ come after the note fields so that we don't mess up the
    // `NoteRecord::from_row` call.
    let mut stmt = transaction.prepare_cached(&format!(
        "
        SELECT {}, rowid FROM notes
        WHERE
            execution_mode = 0 AND consumed = FALSE AND rowid >= ?
        ORDER BY rowid
        LIMIT ?
        ",
        NoteRecord::SELECT_COLUMNS
    ))?;

    let mut rows = stmt.query(params![token.0, limit])?;

    let mut notes = Vec::with_capacity(limit.into());
    while let Some(row) = rows.next()? {
        notes.push(NoteRecord::from_row(row)?);
        // Increment by 1 because we are using rowid >=, and otherwise we would include the last
        // element in the next page as well.
        token.0 = row.get::<_, i64>(11)? + 1;
    }

    Ok((notes, token))
}

#[derive(Default, Debug, Copy, Clone)]
pub struct PaginationToken(i64);

// BLOCK CHAIN QUERIES
// ================================================================================================

/// Insert a [`BlockHeader`] to the DB using the given [Transaction].
///
/// # Returns
///
/// The number of affected rows.
///
/// # Note
///
/// The [Transaction] object is not consumed. It's up to the caller to commit or rollback the
/// transaction.
pub fn insert_block_header(transaction: &Transaction, block_header: &BlockHeader) -> Result<usize> {
    let mut stmt =
        transaction.prepare_cached(insert_sql!(block_headers { block_num, block_header }))?;
    Ok(stmt.execute(params![block_header.block_num().as_u32(), block_header.to_bytes()])?)
}

/// Select a [`BlockHeader`] from the DB by its `block_num` using the given [Connection].
///
/// # Returns
///
/// When `block_number` is [None], the latest block header is returned. Otherwise, the block with
/// the given block height is returned.
pub fn select_block_header_by_block_num(
    conn: &mut Connection,
    block_number: Option<BlockNumber>,
) -> Result<Option<BlockHeader>> {
    let mut stmt;
    let mut rows = if let Some(block_number) = block_number {
        stmt =
            conn.prepare_cached("SELECT block_header FROM block_headers WHERE block_num = ?1")?;
        stmt.query([block_number.as_u32()])?
    } else {
        stmt = conn.prepare_cached(
            "SELECT block_header FROM block_headers ORDER BY block_num DESC LIMIT 1",
        )?;
        stmt.query([])?
    };

    match rows.next()? {
        Some(row) => {
            let data = row.get_ref(0)?.as_blob()?;
            Ok(Some(BlockHeader::read_from_bytes(data)?))
        },
        None => Ok(None),
    }
}

/// Select all the given block headers from the DB using the given [Connection].
///
/// # Note
///
/// Only returns the block headers that are actually present.
///
/// # Returns
///
/// A vector of [`BlockHeader`] or an error.
pub fn select_block_headers(
    conn: &mut Connection,
    blocks: impl Iterator<Item = BlockNumber> + Send,
) -> Result<Vec<BlockHeader>> {
    let blocks: Vec<Value> = blocks.map(|b| b.as_u32().into()).collect();

    let mut headers = Vec::with_capacity(blocks.len());
    let mut stmt = conn
        .prepare_cached("SELECT block_header FROM block_headers WHERE block_num IN rarray(?1);")?;
    let mut rows = stmt.query(params![Rc::new(blocks)])?;

    while let Some(row) = rows.next()? {
        let header = row.get_ref(0)?.as_blob()?;
        let header = BlockHeader::read_from_bytes(header)?;
        headers.push(header);
    }

    Ok(headers)
}

/// Select all block headers from the DB using the given [Connection].
///
/// # Returns
///
/// A vector of [`BlockHeader`] or an error.
pub fn select_all_block_headers(conn: &mut Connection) -> Result<Vec<BlockHeader>> {
    let mut stmt =
        conn.prepare_cached("SELECT block_header FROM block_headers ORDER BY block_num ASC;")?;
    let mut rows = stmt.query([])?;
    let mut result = vec![];
    while let Some(row) = rows.next()? {
        let block_header_data = row.get_ref(0)?.as_blob()?;
        let block_header = BlockHeader::read_from_bytes(block_header_data)?;
        result.push(block_header);
    }

    Ok(result)
}

// TRANSACTIONS QUERIES
// ================================================================================================

/// Insert transactions to the DB using the given [Transaction].
///
/// # Returns
///
/// The number of affected rows.
///
/// # Note
///
/// The [Transaction] object is not consumed. It's up to the caller to commit or rollback the
/// transaction.
pub fn insert_transactions(
    transaction: &Transaction,
    block_num: BlockNumber,
    accounts: &[BlockAccountUpdate],
) -> Result<usize> {
    let mut stmt = transaction.prepare_cached(insert_sql!(transactions {
        transaction_id,
        account_id,
        block_num
    }))?;
    let mut count = 0;
    for update in accounts {
        let account_id = update.account_id();
        for transaction_id in update.transactions() {
            count += stmt.execute(params![
                transaction_id.to_bytes(),
                account_id.to_bytes(),
                block_num.as_u32()
            ])?;
        }
    }
    Ok(count)
}

/// Select transaction IDs from the DB using the given [Connection], filtered by account IDS,
/// given that the account updates were done between `(block_start, block_end]`.
///
/// # Returns
///
/// The vector of [`RpoDigest`] with the transaction IDs.
pub fn select_transactions_by_accounts_and_block_range(
    conn: &mut Connection,
    block_start: BlockNumber,
    block_end: BlockNumber,
    account_ids: &[AccountId],
) -> Result<Vec<TransactionSummary>> {
    let account_ids: Vec<Value> = account_ids
        .iter()
        .copied()
        .map(|account_id| account_id.to_bytes().into())
        .collect();

    let mut stmt = conn.prepare_cached(
        "
        SELECT
            account_id,
            block_num,
            transaction_id
        FROM
            transactions
        WHERE
            block_num > ?1 AND
            block_num <= ?2 AND
            account_id IN rarray(?3)
        ORDER BY
            transaction_id ASC
    ",
    )?;

    let mut rows =
        stmt.query(params![block_start.as_u32(), block_end.as_u32(), Rc::new(account_ids)])?;

    let mut result = vec![];
    while let Some(row) = rows.next()? {
        let account_id = read_from_blob_column(row, 0)?;
        let block_num = read_block_number(row, 1)?;
        let transaction_id_data = row.get_ref(2)?.as_blob()?;
        let transaction_id = TransactionId::read_from_bytes(transaction_id_data)?;

        result.push(TransactionSummary { account_id, block_num, transaction_id });
    }

    Ok(result)
}

// STATE SYNC
// ================================================================================================

/// Loads the state necessary for a state sync.
pub fn get_state_sync(
    conn: &mut Connection,
    block_num: BlockNumber,
    account_ids: &[AccountId],
    note_tag_prefixes: &[u32],
) -> Result<StateSyncUpdate, StateSyncError> {
    let notes = select_notes_since_block_by_tag_and_sender(
        conn,
        note_tag_prefixes,
        account_ids,
        block_num,
    )?;

    let block_header =
        select_block_header_by_block_num(conn, notes.first().map(|note| note.block_num))?
            .ok_or(StateSyncError::EmptyBlockHeadersTable)?;

    let account_updates =
        select_accounts_by_block_range(conn, block_num, block_header.block_num(), account_ids)?;

    let transactions = select_transactions_by_accounts_and_block_range(
        conn,
        block_num,
        block_header.block_num(),
        account_ids,
    )?;

    Ok(StateSyncUpdate {
        notes,
        block_header,
        account_updates,
        transactions,
    })
}

// NOTE SYNC
// ================================================================================================

/// Loads the data necessary for a note sync.
pub fn get_note_sync(
    conn: &mut Connection,
    block_num: BlockNumber,
    note_tags: &[u32],
) -> Result<NoteSyncUpdate, NoteSyncError> {
    let notes = select_notes_since_block_by_tag_and_sender(conn, note_tags, &[], block_num)?;

    let block_header =
        select_block_header_by_block_num(conn, notes.first().map(|note| note.block_num))?
            .ok_or(NoteSyncError::EmptyBlockHeadersTable)?;

    Ok(NoteSyncUpdate { notes, block_header })
}

// APPLY BLOCK
// ================================================================================================

/// Updates the DB with the state of a new block.
///
/// # Returns
///
/// The number of affected rows in the DB.
pub fn apply_block(
    transaction: &Transaction,
    block_header: &BlockHeader,
    notes: &[(NoteRecord, Option<Nullifier>)],
    nullifiers: &[Nullifier],
    accounts: &[BlockAccountUpdate],
) -> Result<usize> {
    let mut count = 0;
    count += insert_block_header(transaction, block_header)?;
    count += insert_notes(transaction, notes)?;
    count += upsert_accounts(transaction, accounts, block_header.block_num())?;
    count += insert_transactions(transaction, block_header.block_num(), accounts)?;
    count += insert_nullifiers_for_block(transaction, nullifiers, block_header.block_num())?;
    Ok(count)
}
