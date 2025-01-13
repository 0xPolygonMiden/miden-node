//! Wrapper functions for SQL statements.

pub(crate) mod utils;

use std::{
    borrow::Cow,
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    rc::Rc,
};

use miden_node_proto::domain::accounts::{AccountInfo, AccountSummary};
use miden_objects::{
    accounts::{
        delta::AccountUpdateDetails, Account, AccountDelta, AccountId, AccountStorageDelta,
        AccountVaultDelta, FungibleAssetDelta, NonFungibleAssetDelta, NonFungibleDeltaAction,
        StorageMapDelta,
    },
    assets::{Asset, FungibleAsset},
    block::{BlockAccountUpdate, BlockNoteIndex},
    crypto::hash::rpo::RpoDigest,
    notes::{NoteId, NoteInclusionProof, NoteMetadata, NoteType, Nullifier},
    utils::serde::{Deserializable, Serializable},
    BlockHeader, EMPTY_WORD,
};
use rusqlite::{params, types::Value, Connection, Transaction};
use utils::read_from_blob_column;

use super::{
    NoteRecord, NoteSyncRecord, NoteSyncUpdate, NullifierInfo, Result, StateSyncUpdate,
    TransactionSummary,
};
use crate::{
    db::sql::utils::{
        account_info_from_row, account_summary_from_row, apply_delta, column_value_as_u64,
        get_nullifier_prefix, insert_sql, read_nullable_from_blob_column, subst, u64_to_value,
    },
    errors::{DatabaseError, NoteSyncError, StateSyncError},
    types::BlockNumber,
};
// ACCOUNT QUERIES
// ================================================================================================

/// Select all accounts from the DB using the given [Connection].
///
/// # Returns
///
/// A vector with accounts, or an error.
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
        accounts.push(account_info_from_row(row)?)
    }
    Ok(accounts)
}

/// Select all account hashes from the DB using the given [Connection].
///
/// # Returns
///
/// The vector with the account ID and corresponding hash, or an error.
pub fn select_all_account_hashes(conn: &mut Connection) -> Result<Vec<(AccountId, RpoDigest)>> {
    let mut stmt = conn
        .prepare_cached("SELECT account_id, account_hash FROM accounts ORDER BY block_num ASC;")?;
    let mut rows = stmt.query([])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let account_id = read_from_blob_column(row, 0)?;
        let account_hash = read_from_blob_column(row, 1)?;

        result.push((account_id, account_hash));
    }

    Ok(result)
}

/// Select [AccountSummary] from the DB using the given [Connection], given that the latest account
/// update was done between `(block_start, block_end]`.
///
/// # Returns
///
/// The vector of [AccountSummary] with the matching accounts.
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
    let mut rows = stmt.query(params![block_start, block_end, Rc::new(account_ids)])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(account_summary_from_row(row)?)
    }

    Ok(result)
}

/// Select the latest account details by account ID from the DB using the given [Connection].
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

/// Select the latest accounts summary (with optional details for public accounts) filtered by IDs
/// from the DB using the given [Connection].
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
        result.push(account_info_from_row(row)?)
    }

    Ok(result)
}

/// Computes public account states for the particular block number using the given [Connection].
///
/// # Returns
///
/// Account states vector, or an error.
#[cfg_attr(not(test), expect(dead_code))] // TODO: remove once the function is used
pub fn compute_old_account_states(
    conn: &mut Connection,
    account_ids: &[AccountId],
    block_number: BlockNumber,
) -> Result<Vec<Account>> {
    #[derive(Default)]
    struct Accounts(BTreeMap<AccountId, Account>);

    impl Accounts {
        fn try_get_mut(&mut self, id: &AccountId) -> Result<&mut Account> {
            self.0.get_mut(id).ok_or_else(|| {
                DatabaseError::DataCorrupted(format!("Account not found in DB: {id}"))
            })
        }

        fn update_asset(
            &mut self,
            account_id: &AccountId,
            asset: Asset,
            is_remove: bool,
        ) -> Result<()> {
            let account = self.try_get_mut(account_id)?;

            if is_remove {
                account.vault_mut().remove_asset(asset)
            } else {
                account.vault_mut().add_asset(asset)
            }
            .map_err(|err| DatabaseError::DataCorrupted(err.to_string()))?;

            Ok(())
        }
    }

    let account_ids = Rc::new(
        account_ids
            .iter()
            .map(|id| {
                if !id.is_public() {
                    return Err(DatabaseError::AccountNotOnChain(*id));
                }
                Ok(id.to_bytes().into())
            })
            .collect::<Result<Vec<Value>>>()?,
    );

    // Gathering data from different tables to a single accounts map.
    let mut accounts = Accounts::default();
    // Fence for possible situation, if new deltas were added during subsequent requests
    let mut latest_block_number: BlockNumber = 0;

    // *** Query latest states with calculated nonces ***

    // We select states with possible nonce updates in order to (re)construct `Account` with final
    // nonce, since we're not able to decrease nonce in `Account`.
    let mut select_latest_states_stmt = conn.prepare_cached(
        "
        SELECT a.account_id, a.block_num, a.details, d.nonce
        FROM accounts a LEFT JOIN account_deltas d ON
            a.account_id = d.account_id AND
            d.block_num = (
                SELECT MAX(block_num)
                FROM account_deltas
                WHERE
                    block_num <= ?2 AND
                    account_id = a.account_id
            )
        WHERE a.account_id IN rarray(?1)",
    )?;
    let mut rows =
        select_latest_states_stmt.query(params![Rc::clone(&account_ids), block_number])?;
    while let Some(row) = rows.next()? {
        let account_details = row.get_ref(2)?.as_blob_or_null()?;
        if let Some(details) = account_details {
            let block_number: BlockNumber = row.get(1)?;
            if block_number > latest_block_number {
                latest_block_number = block_number;
            }
            let account = Account::read_from_bytes(details)?;
            let old_nonce = row.get::<_, Option<u64>>(3)?;

            // If there is updated (old) nonce, reconstruct loaded details with the nonce.
            let (id, account) = if let Some(old_nonce) = old_nonce {
                let (id, vault, storage, code, _nonce) = account.into_parts();
                let nonce = old_nonce.try_into().map_err(DatabaseError::DataCorrupted)?;

                (id, Account::from_parts(id, vault, storage, code, nonce))
            } else {
                (read_from_blob_column(row, 0)?, account)
            };

            accounts.0.insert(id, account);
        }
    }

    // *** Query and apply storage slot updates to the latest state ***

    let mut select_storage_slot_updates_stmt = conn.prepare_cached(
        "
        SELECT new.account_id, new.slot, old.value
        FROM account_storage_slot_updates new LEFT JOIN account_storage_slot_updates old ON
            old.account_id = new.account_id AND
            old.slot = new.slot AND
            old.block_num = (
                SELECT MAX(block_num)
                FROM account_storage_slot_updates
                WHERE block_num <= ?2 AND account_id = old.account_id AND slot = old.slot
            )
        WHERE
            new.account_id IN rarray(?1) AND
            new.block_num = (
                SELECT MIN(block_num)
                FROM account_storage_slot_updates
                WHERE
                    (block_num BETWEEN ?2 + 1 AND ?3) AND
                    new.account_id = account_id AND
                    new.slot = slot
            )",
    )?;
    let mut rows = select_storage_slot_updates_stmt.query(params![
        Rc::clone(&account_ids),
        block_number,
        latest_block_number,
    ])?;
    while let Some(row) = rows.next()? {
        let account_id: AccountId = read_from_blob_column(row, 0)?;
        let slot: u8 = row.get(1)?;
        let value = read_nullable_from_blob_column(row, 2)?.unwrap_or(EMPTY_WORD);
        let account = accounts.try_get_mut(&account_id)?;
        account.storage_mut().set_item(slot, value)?;
    }

    // *** Query and apply storage map updates to the latest state ***

    let mut select_storage_map_updates_stmt = conn.prepare_cached(
        "
        SELECT new.account_id, new.slot, new.key, old.value
        FROM account_storage_map_updates new LEFT JOIN account_storage_map_updates old ON
            old.account_id = new.account_id AND
            old.slot = new.slot AND
            old.key = new.key AND
            old.block_num = (
                SELECT MAX(block_num)
                FROM account_storage_map_updates
                WHERE
                    block_num <= ?2 AND
                    account_id = old.account_id AND
                    slot = old.slot AND
                    key = old.key
            )
        WHERE
            new.account_id IN rarray(?1) AND
            new.block_num = (
                SELECT MIN(block_num)
                FROM account_storage_map_updates
                WHERE
                    (block_num BETWEEN ?2 + 1 AND ?3) AND
                    new.account_id = account_id AND
                    new.slot = slot AND
                    new.key = key
            )",
    )?;
    let mut rows = select_storage_map_updates_stmt.query(params![
        Rc::clone(&account_ids),
        block_number,
        latest_block_number,
    ])?;
    while let Some(row) = rows.next()? {
        let account_id: AccountId = read_from_blob_column(row, 0)?;
        let slot: u8 = row.get(1)?;
        let key = read_from_blob_column(row, 2)?;
        let value = read_nullable_from_blob_column(row, 3)?.unwrap_or(EMPTY_WORD);
        let account = accounts.try_get_mut(&account_id)?;
        account.storage_mut().set_map_item(slot, key, value)?;
    }

    // *** Query and add/subtract fungible asset updates to the latest state ***

    let mut select_fungible_asset_deltas_stmt = conn.prepare_cached(
        "
        SELECT account_id, faucet_id, SUM(delta) AS delta
        FROM account_fungible_asset_deltas
        WHERE account_id IN rarray(?1) AND (block_num BETWEEN ?2 + 1 AND ?3)
        GROUP BY account_id, faucet_id
        HAVING delta != 0",
    )?;
    let mut rows = select_fungible_asset_deltas_stmt.query(params![
        Rc::clone(&account_ids),
        block_number,
        latest_block_number,
    ])?;
    while let Some(row) = rows.next()? {
        let account_id = read_from_blob_column(row, 0)?;
        let faucet_id = read_from_blob_column(row, 1)?;
        let amount: i64 = row.get(2)?;
        let asset = Asset::Fungible(
            FungibleAsset::new(faucet_id, amount.unsigned_abs())
                .map_err(|err| DatabaseError::DataCorrupted(err.to_string()))?,
        );
        accounts.update_asset(&account_id, asset, amount >= 0)?; // Inverse logic
    }

    // *** Query and add/remove non-fungible asset updates to the latest state ***

    let mut select_latest_non_fungible_assets_stmt = conn.prepare_cached(
        "
        SELECT account_id, vault_key, is_remove, block_num
        FROM account_non_fungible_asset_updates
        WHERE account_id IN rarray(?1) AND (block_num BETWEEN ?2 + 1 AND ?3)
        ORDER BY account_id, block_num DESC",
    )?;
    let mut rows = select_latest_non_fungible_assets_stmt.query(params![
        Rc::clone(&account_ids),
        block_number,
        latest_block_number,
    ])?;
    while let Some(row) = rows.next()? {
        let account_id = read_from_blob_column(row, 0)?;
        let asset = read_from_blob_column(row, 1)?;
        let is_remove: bool = !row.get(2)?; // Inverse logic
        accounts.update_asset(&account_id, Asset::NonFungible(asset), is_remove)?;
    }

    // Converting gathered data to vector of `Account` structures.
    Ok(accounts.0.into_values().collect())
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
        .query_row(params![account_id, block_start, block_end], |row| row.get::<_, u64>(0))
    {
        Ok(nonce) => nonce.try_into().map_err(DatabaseError::InvalidFelt)?,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let mut storage_scalars = BTreeMap::new();
    let mut rows = select_slot_updates_stmt.query(params![account_id, block_start, block_end])?;
    while let Some(row) = rows.next()? {
        let slot = row.get(0)?;
        let value = read_from_blob_column(row, 1)?;
        storage_scalars.insert(slot, value);
    }

    let mut storage_maps = BTreeMap::new();
    let mut rows =
        select_storage_map_updates_stmt.query(params![account_id, block_start, block_end])?;
    while let Some(row) = rows.next()? {
        let slot = row.get(0)?;
        let key = read_from_blob_column(row, 1)?;
        let value = read_from_blob_column(row, 2)?;

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
    let mut rows =
        select_fungible_asset_deltas_stmt.query(params![account_id, block_start, block_end])?;
    while let Some(row) = rows.next()? {
        let faucet_id: AccountId = read_from_blob_column(row, 0)?;
        let value = row.get(1)?;
        fungible.insert(faucet_id, value);
    }

    let mut non_fungible_delta = NonFungibleAssetDelta::default();
    let mut rows = select_non_fungible_asset_updates_stmt.query(params![
        account_id,
        block_start,
        block_end
    ])?;
    while let Some(row) = rows.next()? {
        let asset = read_from_blob_column(row, 1)?;
        let action: usize = row.get(2)?;
        match action {
            0 => non_fungible_delta.add(asset)?,
            1 => non_fungible_delta.remove(asset)?,
            _ => {
                return Err(DatabaseError::DataCorrupted(format!(
                    "Invalid non-fungible asset delta action: {action}"
                )))
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
    let mut upsert_stmt = transaction.prepare_cached(
        "INSERT OR REPLACE INTO accounts (account_id, account_hash, block_num, details) VALUES (?1, ?2, ?3, ?4);",
    )?;
    let mut select_details_stmt =
        transaction.prepare_cached("SELECT details FROM accounts WHERE account_id = ?1;")?;

    let mut count = 0;
    for update in accounts.iter() {
        let account_id = update.account_id();
        let (full_account, insert_delta) = match update.details() {
            AccountUpdateDetails::Private => (None, None),
            AccountUpdateDetails::New(account) => {
                debug_assert_eq!(account_id, account.id());

                if account.hash() != update.new_state_hash() {
                    return Err(DatabaseError::AccountHashesMismatch {
                        calculated: account.hash(),
                        expected: update.new_state_hash(),
                    });
                }

                let insert_delta = AccountDelta::from(account.clone());

                (Some(Cow::Borrowed(account)), Some(Cow::Owned(insert_delta)))
            },
            AccountUpdateDetails::Delta(delta) => {
                let mut rows = select_details_stmt.query(params![account_id.to_bytes()])?;
                let Some(row) = rows.next()? else {
                    return Err(DatabaseError::AccountNotFoundInDb(account_id));
                };

                let account =
                    apply_delta(account_id, &row.get_ref(0)?, delta, &update.new_state_hash())?;

                (Some(Cow::Owned(account)), Some(Cow::Borrowed(delta)))
            },
        };

        let inserted = upsert_stmt.execute(params![
            account_id.to_bytes(),
            update.new_state_hash().to_bytes(),
            block_num,
            full_account.as_ref().map(|account| account.to_bytes()),
        ])?;

        debug_assert_eq!(inserted, 1);

        if let Some(delta) = insert_delta {
            insert_account_delta(transaction, account_id, block_num, &delta)?;
        }

        count += inserted;
    }

    Ok(count)
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
        block_number,
        delta.nonce().map(Into::<u64>::into).unwrap_or_default()
    ])?;

    for (&slot, value) in delta.storage().values() {
        insert_slot_update_stmt.execute(params![
            account_id.to_bytes(),
            block_number,
            slot,
            value.to_bytes()
        ])?;
    }

    for (&slot, map_delta) in delta.storage().maps() {
        for (key, value) in map_delta.leaves() {
            insert_storage_map_update_stmt.execute(params![
                account_id.to_bytes(),
                block_number,
                slot,
                key.to_bytes(),
                value.to_bytes(),
            ])?;
        }
    }

    for (&faucet_id, &delta) in delta.vault().fungible().iter() {
        insert_fungible_asset_delta_stmt.execute(params![
            account_id.to_bytes(),
            block_number,
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
            block_number,
            asset.to_bytes(),
            is_remove,
        ])?;
    }

    Ok(())
}

// NULLIFIER QUERIES
// ================================================================================================

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
    nullifiers: &[Nullifier],
    block_num: BlockNumber,
) -> Result<usize> {
    let mut stmt = transaction.prepare_cached(
        "INSERT INTO nullifiers (nullifier, nullifier_prefix, block_num) VALUES (?1, ?2, ?3);",
    )?;

    let mut count = 0;
    for nullifier in nullifiers.iter() {
        count +=
            stmt.execute(params![nullifier.to_bytes(), get_nullifier_prefix(nullifier), block_num])?
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
        let nullifier = read_from_blob_column(row, 0)?;
        let block_number = row.get(1)?;
        result.push((nullifier, block_number));
    }
    Ok(result)
}

/// Select nullifiers created between `(block_start, block_end]` that also match the
/// `nullifier_prefixes` filter using the given [Connection].
///
/// Each value of the `nullifier_prefixes` is only the 16 most significant bits of the nullifier of
/// interest to the client. This hides the details of the specific nullifier being requested.
///
/// # Returns
///
/// A vector of [NullifierInfo] with the nullifiers and the block height at which they were
/// created, or an error.
pub fn select_nullifiers_by_block_range(
    conn: &mut Connection,
    block_start: BlockNumber,
    block_end: BlockNumber,
    nullifier_prefixes: &[u32],
) -> Result<Vec<NullifierInfo>> {
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
            block_num > ?1 AND
            block_num <= ?2 AND
            nullifier_prefix IN rarray(?3)
        ORDER BY
            block_num ASC
    ",
    )?;

    let mut rows = stmt.query(params![block_start, block_end, Rc::new(nullifier_prefixes)])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let nullifier = read_from_blob_column(row, 0)?;
        let block_num = row.get(1)?;
        result.push(NullifierInfo { nullifier, block_num });
    }
    Ok(result)
}

/// Select nullifiers created that match the `nullifier_prefixes` filter using the given
/// [Connection].
///
/// Each value of the `nullifier_prefixes` is only the `prefix_len` most significant bits
/// of the nullifier of interest to the client. This hides the details of the specific
/// nullifier being requested. Currently the only supported prefix length is 16 bits.
///
/// # Returns
///
/// A vector of [NullifierInfo] with the nullifiers and the block height at which they were
/// created, or an error.
pub fn select_nullifiers_by_prefix(
    conn: &mut Connection,
    prefix_len: u32,
    nullifier_prefixes: &[u32],
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
            nullifier_prefix IN rarray(?1)
        ORDER BY
            block_num ASC
    ",
    )?;

    let mut rows = stmt.query(params![Rc::new(nullifier_prefixes)])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let nullifier = read_from_blob_column(row, 0)?;
        let block_num = row.get(1)?;
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
pub fn select_all_notes(conn: &mut Connection) -> Result<Vec<NoteRecord>> {
    let mut stmt = conn.prepare_cached(
        "
        SELECT
            block_num,
            batch_index,
            note_index,
            note_id,
            note_type,
            sender,
            tag,
            aux,
            execution_hint,
            merkle_path,
            details
        FROM
            notes
        ORDER BY
            block_num ASC;
        ",
    )?;
    let mut rows = stmt.query([])?;

    let mut notes = vec![];
    while let Some(row) = rows.next()? {
        let note_id = read_from_blob_column(row, 3)?;
        let note_type = row.get::<_, u8>(4)?.try_into()?;
        let sender = read_from_blob_column(row, 5)?;
        let tag: u32 = row.get(6)?;
        let aux: u64 = row.get(7)?;
        let aux = aux.try_into().map_err(DatabaseError::InvalidFelt)?;
        let execution_hint = column_value_as_u64(row, 8)?;
        let merkle_path = read_from_blob_column(row, 9)?;
        let details_data = row.get_ref(10)?.as_blob_or_null()?;
        let details = details_data.map(<Vec<u8>>::read_from_bytes).transpose()?;

        let metadata =
            NoteMetadata::new(sender, note_type, tag.into(), execution_hint.try_into()?, aux)?;

        notes.push(NoteRecord {
            block_num: row.get(0)?,
            note_index: BlockNoteIndex::new(row.get(1)?, row.get(2)?)?,
            note_id,
            metadata,
            details,
            merkle_path,
        })
    }
    Ok(notes)
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
pub fn insert_notes(transaction: &Transaction, notes: &[NoteRecord]) -> Result<usize> {
    let mut stmt = transaction.prepare_cached(insert_sql!(notes {
        block_num,
        batch_index,
        note_index,
        note_id,
        note_type,
        sender,
        tag,
        aux,
        execution_hint,
        merkle_path,
        details,
    }))?;

    let mut count = 0;
    for note in notes.iter() {
        let details = note.details.as_ref().map(|details| details.to_bytes());
        count += stmt.execute(params![
            note.block_num,
            note.note_index.batch_idx(),
            note.note_index.note_idx_in_batch(),
            note.note_id.to_bytes(),
            note.metadata.note_type() as u8,
            note.metadata.sender().to_bytes(),
            note.metadata.tag().inner(),
            u64_to_value(note.metadata.aux().into()),
            Into::<u64>::into(note.metadata.execution_hint()),
            note.merkle_path.to_bytes(),
            details,
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
    let mut rows = stmt.query(params![Rc::new(tags), Rc::new(account_ids), block_num])?;

    let mut res = Vec::new();
    while let Some(row) = rows.next()? {
        let block_num = row.get(0)?;
        let note_index = BlockNoteIndex::new(row.get(1)?, row.get(2)?)?;
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

/// Select Note's matching the NoteId using the given [Connection].
///
/// # Returns
///
/// - Empty vector if no matching `note`.
/// - Otherwise, notes which `note_id` matches the `NoteId` as bytes.
pub fn select_notes_by_id(conn: &mut Connection, note_ids: &[NoteId]) -> Result<Vec<NoteRecord>> {
    let note_ids: Vec<Value> = note_ids.iter().map(|id| id.to_bytes().into()).collect();

    let mut stmt = conn.prepare_cached(
        "
        SELECT
            block_num,
            batch_index,
            note_index,
            note_id,
            note_type,
            sender,
            tag,
            aux,
            execution_hint,
            merkle_path,
            details
        FROM
            notes
        WHERE
            note_id IN rarray(?1)
        ",
    )?;
    let mut rows = stmt.query(params![Rc::new(note_ids)])?;

    let mut notes = Vec::new();
    while let Some(row) = rows.next()? {
        let note_id: NoteId = read_from_blob_column(row, 3)?;
        let merkle_path = read_from_blob_column(row, 9)?;

        let details_data = row.get_ref(10)?.as_blob_or_null()?;
        let details = details_data.map(<Vec<u8>>::read_from_bytes).transpose()?;

        let note_type = row.get::<_, u8>(4)?.try_into()?;
        let sender = read_from_blob_column(row, 5)?;
        let tag: u32 = row.get(6)?;
        let aux: u64 = row.get(7)?;
        let aux = aux.try_into().map_err(DatabaseError::InvalidFelt)?;
        let execution_hint = column_value_as_u64(row, 8)?;

        let metadata =
            NoteMetadata::new(sender, note_type, tag.into(), execution_hint.try_into()?, aux)?;

        notes.push(NoteRecord {
            block_num: row.get(0)?,
            note_index: BlockNoteIndex::new(row.get(1)?, row.get(2)?)?,
            details,
            note_id: note_id.into(),
            metadata,
            merkle_path,
        })
    }

    Ok(notes)
}

/// Select note inclusion proofs matching the NoteId, using the given [Connection].
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
        let block_num = row.get(0)?;
        let note_id = read_from_blob_column(row, 1)?;
        let batch_index = row.get(2)?;
        let note_index = row.get(3)?;
        let node_index_in_block = BlockNoteIndex::new(batch_index, note_index)?.leaf_index_value();
        let merkle_path = read_from_blob_column(row, 4)?;

        let proof = NoteInclusionProof::new(block_num, node_index_in_block, merkle_path)?;

        result.insert(note_id, proof);
    }

    Ok(result)
}

// BLOCK CHAIN QUERIES
// ================================================================================================

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
pub fn insert_block_header(transaction: &Transaction, block_header: &BlockHeader) -> Result<usize> {
    let mut stmt = transaction
        .prepare_cached("INSERT INTO block_headers (block_num, block_header) VALUES (?1, ?2);")?;
    Ok(stmt.execute(params![block_header.block_num(), block_header.to_bytes()])?)
}

/// Select a [BlockHeader] from the DB by its `block_num` using the given [Connection].
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
    let mut rows = match block_number {
        Some(block_number) => {
            stmt =
                conn.prepare_cached("SELECT block_header FROM block_headers WHERE block_num = ?1")?;
            stmt.query([block_number])?
        },
        None => {
            stmt = conn.prepare_cached(
                "SELECT block_header FROM block_headers ORDER BY block_num DESC LIMIT 1",
            )?;
            stmt.query([])?
        },
    };

    match rows.next()? {
        Some(row) => Ok(Some(read_from_blob_column(row, 0)?)),
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
/// A vector of [BlockHeader] or an error.
pub fn select_block_headers(
    conn: &mut Connection,
    blocks: Vec<BlockNumber>,
) -> Result<Vec<BlockHeader>> {
    let mut headers = Vec::with_capacity(blocks.len());

    let blocks: Vec<Value> = blocks.iter().copied().map(Into::into).collect();
    let mut stmt = conn
        .prepare_cached("SELECT block_header FROM block_headers WHERE block_num IN rarray(?1);")?;
    let mut rows = stmt.query(params![Rc::new(blocks)])?;

    while let Some(row) = rows.next()? {
        headers.push(read_from_blob_column(row, 0)?);
    }

    Ok(headers)
}

/// Select all block headers from the DB using the given [Connection].
///
/// # Returns
///
/// A vector of [BlockHeader] or an error.
pub fn select_all_block_headers(conn: &mut Connection) -> Result<Vec<BlockHeader>> {
    let mut stmt =
        conn.prepare_cached("SELECT block_header FROM block_headers ORDER BY block_num ASC;")?;
    let mut rows = stmt.query([])?;
    let mut result = vec![];
    while let Some(row) = rows.next()? {
        result.push(read_from_blob_column(row, 0)?);
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
    let mut stmt = transaction.prepare_cached(
        "INSERT INTO transactions (transaction_id, account_id, block_num) VALUES (?1, ?2, ?3);",
    )?;
    let mut count = 0;
    for update in accounts {
        let account_id = update.account_id();
        for transaction_id in update.transactions() {
            count +=
                stmt.execute(params![transaction_id.to_bytes(), account_id.to_bytes(), block_num])?
        }
    }
    Ok(count)
}

/// Select transaction IDs from the DB using the given [Connection], filtered by account IDS,
/// given that the account updates were done between `(block_start, block_end]`.
///
/// # Returns
///
/// The vector of [RpoDigest] with the transaction IDs.
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

    let mut rows = stmt.query(params![block_start, block_end, Rc::new(account_ids)])?;

    let mut result = vec![];
    while let Some(row) = rows.next()? {
        let account_id = read_from_blob_column(row, 0)?;
        let block_num = row.get(1)?;
        let transaction_id = read_from_blob_column(row, 2)?;

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
    nullifier_prefixes: &[u32],
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

    // TODO: What if account was also changed after requested block number? In this case we will
    //       miss the update (since `accounts` table stores only latest account states,
    //       the corresponding record in the table will be updated to the state beyond the requested
    //       block range).
    let account_updates =
        select_accounts_by_block_range(conn, block_num, block_header.block_num(), account_ids)?;

    let transactions = select_transactions_by_accounts_and_block_range(
        conn,
        block_num,
        block_header.block_num(),
        account_ids,
    )?;

    let nullifiers = select_nullifiers_by_block_range(
        conn,
        block_num,
        block_header.block_num(),
        nullifier_prefixes,
    )?;

    Ok(StateSyncUpdate {
        notes,
        block_header,
        account_updates,
        transactions,
        nullifiers,
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
    notes: &[NoteRecord],
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
