use std::fs::{self, create_dir_all};

use anyhow::anyhow;
use deadpool_sqlite::{Config as SqliteConfig, Pool, Runtime};
use miden_crypto::hash::rpo::RpoDigest;
use miden_node_proto::{
    block_header,
    digest::Digest,
    note::Note,
    responses::{AccountHashUpdate, NullifierUpdate},
};
use miden_node_utils::genesis::GenesisState;
use miden_objects::BlockHeader;
use rusqlite::vtab::array;
use tokio::sync::oneshot;
use tracing::{info, span, Level};

use crate::{
    config::StoreConfig,
    migrations,
    types::{AccountId, BlockNumber},
    COMPONENT,
};

mod sql;

#[cfg(test)]
mod tests;

pub struct Db {
    pool: Pool,
}

#[derive(Debug, PartialEq)]
pub struct StateSyncUpdate {
    pub notes: Vec<Note>,
    pub block_header: block_header::BlockHeader,
    pub chain_tip: BlockNumber,
    pub account_updates: Vec<AccountHashUpdate>,
    pub nullifiers: Vec<NullifierUpdate>,
}

impl Db {
    /// Open a connection to the DB, apply any pending migrations, and ensure that the genesis block
    /// is as expected and present in the database.
    pub async fn setup(config: StoreConfig) -> Result<Self, anyhow::Error> {
        if let Some(p) = config.sqlite.parent() {
            create_dir_all(p)?;
        }

        let pool = SqliteConfig::new(config.sqlite.clone()).create_pool(Runtime::Tokio1)?;

        let conn = pool.get().await?;

        info!(
            sqlite = format!("{}", config.sqlite.display()),
            COMPONENT, "Connected to the DB"
        );

        // Feature used to support `IN` and `NOT IN` queries
        conn.interact(|conn| array::load_module(conn))
            .await
            .map_err(|_| anyhow!("Loading carray module failed"))??;

        conn.interact(|conn| migrations::MIGRATIONS.to_latest(conn))
            .await
            .map_err(|_| anyhow!("Migration task failed with a panic"))??;

        let db = Db { pool };
        db.ensure_genesis_block(&config.genesis_filepath).await?;

        Ok(db)
    }

    /// Loads all the nullifiers from the DB.
    pub async fn select_nullifiers(&self) -> Result<Vec<(RpoDigest, BlockNumber)>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(sql::select_nullifiers)
            .await
            .map_err(|_| anyhow!("Get nullifiers task failed with a panic"))?
    }

    /// Search for a [BlockHeader] from the DB by its `block_num`.
    ///
    /// When `block_number` is [None], the latest block header is returned.
    pub async fn select_block_header_by_block_num(
        &self,
        block_number: Option<BlockNumber>,
    ) -> Result<Option<block_header::BlockHeader>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(move |conn| sql::select_block_header_by_block_num(conn, block_number))
            .await
            .map_err(|_| anyhow!("Get block header task failed with a panic"))?
    }

    /// Loads all the block headers from the DB.
    pub async fn select_block_headers(
        &self
    ) -> Result<Vec<block_header::BlockHeader>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(sql::select_block_headers)
            .await
            .map_err(|_| anyhow!("Get block headers task failed with a panic"))?
    }

    /// Loads all the account hashes from the DB.
    pub async fn select_account_hashes(&self) -> Result<Vec<(AccountId, Digest)>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(sql::select_account_hashes)
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

    /// Inserts the data of a new block into the DB.
    ///
    /// `allow_acquire` and `acquire_done` are used to synchronize writes to the DB with writes to
    /// the in-memory trees. Further details available on [super::state::State::apply_block].
    pub async fn apply_block(
        &self,
        allow_acquire: oneshot::Sender<()>,
        acquire_done: oneshot::Receiver<()>,
        block_header: block_header::BlockHeader,
        notes: Vec<Note>,
        nullifiers: Vec<RpoDigest>,
        accounts: Vec<(AccountId, Digest)>,
    ) -> Result<(), anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(move |conn| -> anyhow::Result<()> {
                let span = span!(Level::INFO, COMPONENT, "writing new block data to DB");
                let guard = span.enter();

                let transaction = conn.transaction()?;
                sql::apply_block(&transaction, &block_header, &notes, &nullifiers, &accounts)?;

                let _ = allow_acquire.send(());
                acquire_done.blocking_recv()?;

                transaction.commit()?;

                drop(guard);
                Ok(())
            })
            .await
            .map_err(|_| anyhow!("Apply block task failed with a panic"))??;

        Ok(())
    }

    // HELPERS
    // ---------------------------------------------------------------------------------------------

    /// If the database is empty, generates and stores the genesis block. Otherwise, it ensures that the
    /// genesis block in the database is consistent with the genesis block data in the genesis JSON
    /// file.
    async fn ensure_genesis_block(
        &self,
        genesis_filepath: &str,
    ) -> Result<(), anyhow::Error> {
        let expected_genesis_header: block_header::BlockHeader = {
            let file_contents = fs::read_to_string(genesis_filepath)?;

            let genesis_state: GenesisState = serde_json::from_str(&file_contents)?;
            let block_header: BlockHeader = genesis_state.try_into()?;

            block_header.into()
        };

        let maybe_block_header_in_store = self.select_block_header_by_block_num(Some(0)).await?;

        match maybe_block_header_in_store {
            Some(block_header) => {
                // ensure that expected header is what's also in the store
                if expected_genesis_header != block_header {
                    return Err(anyhow!("block header in store doesn't match block header in genesis file \"{genesis_filepath}\". expected {expected_genesis_header:?}, but store contained {block_header:?}"));
                }
            },
            None => {
                // add genesis header to store

                // FIXME: `Db::apply_block` requires the channels that are set up in `State`.
            },
        }

        Ok(())
    }
}
