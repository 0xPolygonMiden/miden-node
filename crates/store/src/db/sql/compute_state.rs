use std::{collections::BTreeMap, rc::Rc};

use miden_objects::{
    account::{Account, AccountCode, AccountId, AccountStorage, StorageSlot, StorageSlotType},
    asset::{Asset, AssetVault, FungibleAsset, NonFungibleAsset},
    utils::Serializable,
    Felt, Word,
};
use rusqlite::{types::Value, Connection, Row};

use super::utils::{read_from_blob_column, AugmentedStatement};
use crate::{db::Result, errors::DatabaseError, state::StateQueryParams};

pub struct AccountRecord<T> {
    pub account_id: AccountId,
    pub payload: T,
}

pub struct BasicPayload {
    pub nonce: Felt,
    pub code: AccountCode,
    pub storage_layout: Vec<StorageSlotType>,
}

pub struct StorageSlotPayload {
    pub slot: u8,
    pub value: Word,
}

pub struct StorageMapPayload {
    pub slot: u8,
    pub key: Word,
    pub value: Word,
}

pub struct FungibleAssetPayload {
    pub faucet_id: AccountId,
    pub amount: u64,
}

pub struct NonFungibleAssetPayload {
    pub vault_key: NonFungibleAsset,
}

/// Computes public account state for the given block number using the given [Connection].
///
/// # Returns
///
/// Account state, or an error.
pub fn compute_account_states(
    conn: &Connection,
    query_params: StateQueryParams,
) -> Result<BTreeMap<AccountId, Option<Account>>> {
    #[derive(Default)]
    struct Accounts(BTreeMap<AccountId, Option<Account>>);

    impl Accounts {
        fn insert(&mut self, id: AccountId, account: Option<Account>) {
            self.0.insert(id, account);
        }

        fn try_get_mut(&mut self, id: &AccountId) -> crate::db::Result<&mut Account> {
            self.0.get_mut(id).and_then(Option::as_mut).ok_or_else(|| {
                DatabaseError::DataCorrupted(format!("Account not found in DB: {id}"))
            })
        }
    }

    // Gathering data from different tables to single accounts map.
    let mut accounts = Accounts::default();

    let state_queries = StateQueries { query_params };

    // *** Query latest accounts' nonces and codes ***
    let mut stmt = state_queries.query_basic_info(conn)?;
    for row_result in stmt.query()? {
        let AccountRecord { account_id, payload } = row_result?;
        let account = payload
            .map::<Result<Account>, _>(|payload| {
                let storage_slots = payload
                    .storage_layout
                    .into_iter()
                    .map(|slot_type| match slot_type {
                        StorageSlotType::Value => StorageSlot::empty_value(),
                        StorageSlotType::Map => StorageSlot::empty_map(),
                    })
                    .collect();
                Ok(Account::from_parts(
                    account_id,
                    AssetVault::default(),
                    AccountStorage::new(storage_slots)
                        .map_err(|err| DatabaseError::DataCorrupted(err.to_string()))?,
                    payload.code,
                    payload.nonce,
                ))
            })
            .transpose()?;

        accounts.insert(account_id, account);
    }

    // *** Query latest storage slot values ***
    let mut stmt = state_queries.query_latest_storage_slots(conn)?;
    for row_result in stmt.query()? {
        let AccountRecord { account_id, payload } = row_result?;
        let account = accounts.try_get_mut(&account_id)?;
        account.storage_mut().set_item(payload.slot, payload.value)?;
    }

    // *** Query latest storage map values ***
    let mut stmt = state_queries.query_latest_storage_map_values(conn)?;
    for row_result in stmt.query()? {
        let AccountRecord { account_id, payload } = row_result?;
        let account = accounts.try_get_mut(&account_id)?;
        account.storage_mut().set_map_item(payload.slot, payload.key, payload.value)?;
    }

    // *** Calculate fungible asset amounts ***
    let mut stmt = state_queries.query_latest_fungible_assets(conn)?;
    for row_result in stmt.query()? {
        let AccountRecord { account_id, payload } = row_result?;
        let asset = Asset::Fungible(
            FungibleAsset::new(payload.faucet_id, payload.amount)
                .map_err(|err| DatabaseError::DataCorrupted(err.to_string()))?,
        );

        let account = accounts.try_get_mut(&account_id)?;
        account
            .vault_mut()
            .add_asset(asset)
            .map_err(|err| DatabaseError::DataCorrupted(err.to_string()))?;
    }

    // *** Query latest non-fungible asset values ***
    let mut stmt = state_queries.query_latest_non_fungible_assets(conn)?;
    for row_result in stmt.query()? {
        let AccountRecord { account_id, payload } = row_result?;
        let account = accounts.try_get_mut(&account_id)?;
        account
            .vault_mut()
            .add_asset(Asset::NonFungible(payload.vault_key))
            .map_err(|err| DatabaseError::DataCorrupted(err.to_string()))?;
    }

    Ok(accounts.0)
}

struct StateQueries {
    query_params: StateQueryParams,
}

type SqlParams = (Rc<Vec<Value>>, u32);
type MapFieldsCallback<T> = fn(&Row<'_>) -> Result<AccountRecord<T>>;

impl StateQueries {
    fn query_basic_info<'conn>(
        &self,
        conn: &'conn Connection,
    ) -> Result<AugmentedStatement<'conn, SqlParams, MapFieldsCallback<Option<BasicPayload>>>> {
        fn extract_fields(row: &Row) -> Result<AccountRecord<Option<BasicPayload>>> {
            let account_id: AccountId = read_from_blob_column(row, 0)?;
            let payload = row
                .get::<_, Option<u64>>(1)?
                .map::<Result<BasicPayload>, _>(|nonce| {
                    let nonce = nonce.try_into().map_err(DatabaseError::DataCorrupted)?;
                    let code = read_from_blob_column(row, 2)?;
                    let storage_layout = read_from_blob_column(row, 3)?;

                    Ok(BasicPayload { nonce, code, storage_layout })
                })
                .transpose()?;

            Ok(AccountRecord { account_id, payload })
        }

        let statement = conn.prepare_cached(
            "
            SELECT a.account_id, d1.nonce, c.code, p.storage_layout
            FROM accounts a
                LEFT JOIN public_accounts p ON a.account_id = p.account_id
                LEFT JOIN account_codes c ON p.code_hash = c.code_hash
                LEFT JOIN account_deltas d1 ON a.account_id = d1.account_id AND
                    d1.block_num = (
                        SELECT MAX(block_num)
                        FROM account_deltas d2
                        WHERE
                            account_id = d1.account_id AND
                            block_num <= ?2
                    )
            WHERE a.account_id IN rarray(?1)",
        )?;

        Ok(AugmentedStatement::new(statement, self.make_params(), extract_fields))
    }

    fn query_latest_storage_slots<'conn>(
        &self,
        conn: &'conn Connection,
    ) -> Result<AugmentedStatement<'conn, SqlParams, MapFieldsCallback<StorageSlotPayload>>> {
        fn extract_fields(row: &Row) -> Result<AccountRecord<StorageSlotPayload>> {
            let account_id: AccountId = read_from_blob_column(row, 0)?;
            let slot = row.get(1)?;
            let value = read_from_blob_column(row, 2)?;

            Ok(AccountRecord {
                account_id,
                payload: StorageSlotPayload { slot, value },
            })
        }

        let statement = conn.prepare_cached(
            "
            SELECT account_id, slot, value
            FROM account_storage_slot_updates a
            WHERE
                account_id IN rarray(?1) AND
                block_num = (
                    SELECT MAX(block_num)
                    FROM account_storage_slot_updates b
                    WHERE
                        b.block_num <= ?2 AND
                        a.account_id = b.account_id AND
                        a.slot = b.slot
                )",
        )?;

        Ok(AugmentedStatement::new(statement, self.make_params(), extract_fields))
    }

    fn query_latest_storage_map_values<'conn>(
        &self,
        conn: &'conn Connection,
    ) -> Result<AugmentedStatement<'conn, SqlParams, MapFieldsCallback<StorageMapPayload>>> {
        fn extract_fields(row: &Row) -> Result<AccountRecord<StorageMapPayload>> {
            let account_id: AccountId = read_from_blob_column(row, 0)?;
            let slot = row.get(1)?;
            let key = read_from_blob_column(row, 2)?;
            let value = read_from_blob_column(row, 3)?;

            Ok(AccountRecord {
                account_id,
                payload: StorageMapPayload { slot, key, value },
            })
        }

        let statement = conn.prepare_cached(
            "
            SELECT account_id, slot, key, value
            FROM account_storage_map_updates a
            WHERE
                account_id IN rarray(?1) AND
                block_num = (
                    SELECT MAX(block_num)
                    FROM account_storage_map_updates b
                    WHERE
                        b.block_num <= ?2 AND
                        a.account_id = b.account_id AND
                        a.slot = b.slot AND
                        a.key = b.key
                )",
        )?;

        Ok(AugmentedStatement::new(statement, self.make_params(), extract_fields))
    }

    fn query_latest_fungible_assets<'conn>(
        &self,
        conn: &'conn Connection,
    ) -> Result<AugmentedStatement<'conn, SqlParams, MapFieldsCallback<FungibleAssetPayload>>> {
        fn extract_fields(row: &Row) -> Result<AccountRecord<FungibleAssetPayload>> {
            let account_id: AccountId = read_from_blob_column(row, 0)?;
            let faucet_id = read_from_blob_column(row, 1)?;
            let amount = row.get(2)?;

            Ok(AccountRecord {
                account_id,
                payload: FungibleAssetPayload { faucet_id, amount },
            })
        }

        let statement = conn.prepare_cached(
            "
            SELECT account_id, faucet_id, SUM(delta) AS delta
            FROM account_fungible_asset_deltas
            WHERE account_id IN rarray(?1) AND block_num <= ?2
            GROUP BY account_id, faucet_id
            HAVING delta != 0",
        )?;

        Ok(AugmentedStatement::new(statement, self.make_params(), extract_fields))
    }

    fn query_latest_non_fungible_assets<'conn>(
        &self,
        conn: &'conn Connection,
    ) -> Result<AugmentedStatement<'conn, SqlParams, MapFieldsCallback<NonFungibleAssetPayload>>>
    {
        fn extract_fields(row: &Row) -> Result<AccountRecord<NonFungibleAssetPayload>> {
            let account_id: AccountId = read_from_blob_column(row, 0)?;
            let vault_key = read_from_blob_column(row, 1)?;

            Ok(AccountRecord {
                account_id,
                payload: NonFungibleAssetPayload { vault_key },
            })
        }

        let statement = conn.prepare_cached(
            "
            SELECT account_id, vault_key
            FROM account_non_fungible_asset_updates a
            WHERE
                account_id IN rarray(?1) AND
                block_num <= ?2 AND
                is_remove = 0 AND
                NOT EXISTS(
                    SELECT 1
                    FROM account_non_fungible_asset_updates b
                    WHERE
                        b.is_remove = 1 AND
                        b.block_num <= ?2 AND
                        a.account_id = b.account_id AND
                        a.vault_key = b.vault_key AND
                        a.block_num < b.block_num
                )",
        )?;

        Ok(AugmentedStatement::new(statement, self.make_params(), extract_fields))
    }

    fn make_params(&self) -> SqlParams {
        let account_ids = Rc::new(
            self.query_params
                .account_ids()
                .iter()
                .map(|id| id.to_bytes().into())
                .collect::<Vec<Value>>(),
        );

        (account_ids, self.query_params.block_number().as_u32())
    }
}
