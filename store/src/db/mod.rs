use crate::{
    config::StoreConfig,
    migrations,
    types::{AccountId, BlockNumber},
};
use anyhow::anyhow;
use deadpool_sqlite::{Config as SqliteConfig, Pool, Runtime};
use miden_crypto::hash::rpo::RpoDigest;
use miden_node_proto::{
    block_header::BlockHeader,
    digest::Digest,
    note::Note,
    responses::{AccountHashUpdate, NullifierUpdate},
};
use rusqlite::vtab::array;
use std::fs::create_dir_all;
use tracing::info;

mod sql;

#[cfg(test)]
mod tests;

pub struct Db {
    pool: Pool,
}

#[derive(Debug, PartialEq)]
pub struct StateSyncUpdate {
    pub notes: Vec<Note>,
    pub block_header: BlockHeader,
    pub chain_tip: BlockNumber,
    pub account_updates: Vec<AccountHashUpdate>,
    pub nullifiers: Vec<NullifierUpdate>,
}

impl Db {
    /// Open a connection to the DB and apply any pending migrations.
    pub async fn get_conn(config: StoreConfig) -> Result<Self, anyhow::Error> {
        if let Some(p) = config.sqlite.parent() {
            create_dir_all(p)?;
        }

        let pool = SqliteConfig::new(config.sqlite.clone()).create_pool(Runtime::Tokio1)?;

        let conn = pool.get().await?;

        info!(sqlite = format!("{}", config.sqlite.display()), "Connected to the DB");

        // Feature used to support `IN` and `NOT IN` queries
        conn.interact(|conn| array::load_module(conn))
            .await
            .map_err(|_| anyhow!("Loading carray module failed"))??;

        conn.interact(|conn| migrations::MIGRATIONS.to_latest(conn))
            .await
            .map_err(|_| anyhow!("Migration task failed with a panic"))??;

        Ok(Db { pool })
    }

    /// Loads all the nullifiers from the DB.
    pub async fn get_nullifiers(&self) -> Result<Vec<(RpoDigest, BlockNumber)>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(sql::get_nullifiers)
            .await
            .map_err(|_| anyhow!("Get nullifiers task failed with a panic"))?
    }

    /// Search for a [BlockHeader] from the DB by its `block_num`.
    ///
    /// When `block_number` is [None], the latest block header is returned.
    pub async fn get_block_header(
        &self,
        block_number: Option<BlockNumber>,
    ) -> Result<Option<BlockHeader>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(move |conn| sql::get_block_header(conn, block_number))
            .await
            .map_err(|_| anyhow!("Get block header task failed with a panic"))?
    }

    /// Loads all the block headers from the DB.
    pub async fn get_block_headers(&self) -> Result<Vec<BlockHeader>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(sql::get_block_headers)
            .await
            .map_err(|_| anyhow!("Get block headers task failed with a panic"))?
    }

    /// Loads all the account hashes from the DB.
    pub async fn get_account_hashes(&self) -> Result<Vec<(AccountId, Digest)>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(sql::get_account_hashes)
            .await
            .map_err(|_| anyhow!("Get account hashes task failed with a panic"))?
    }

    pub async fn get_state_sync(
        &self,
        block_num: BlockNumber,
        account_ids: &[AccountId],
        note_tag_prefixes: &[u32],
        nullifier_prefixes: &[u32],
    ) -> Result<StateSyncUpdate, anyhow::Error> {
        let account_ids = account_ids.to_vec();
        let note_tag_prefixes = note_tag_prefixes.to_vec();
        let nullifier_prefixes = nullifier_prefixes.to_vec();

        self.pool
            .get()
            .await?
            .interact(move |conn| {
                sql::get_state_sync(
                    conn,
                    block_num,
                    &account_ids,
                    &note_tag_prefixes,
                    &nullifier_prefixes,
                )
            })
            .await
            .map_err(|_| anyhow!("Get account hashes task failed with a panic"))?
    }
}
