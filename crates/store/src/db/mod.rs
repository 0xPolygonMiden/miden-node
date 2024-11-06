use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, create_dir_all},
    sync::Arc,
};

use deadpool_sqlite::{Config as SqliteConfig, Hook, HookError, Pool, Runtime};
use miden_node_proto::{
    domain::accounts::{AccountInfo, AccountSummary},
    generated::note::{Note as NotePb, NoteSyncRecord as NoteSyncRecordPb},
};
use miden_objects::{
    accounts::AccountDelta,
    block::{Block, BlockNoteIndex},
    crypto::{hash::rpo::RpoDigest, merkle::MerklePath, utils::Deserializable},
    notes::{NoteId, NoteInclusionProof, NoteMetadata, Nullifier},
    transaction::TransactionId,
    utils::Serializable,
    BlockHeader, GENESIS_BLOCK,
};
use rusqlite::vtab::array;
use tokio::sync::oneshot;
use tracing::{info, info_span, instrument};

use crate::{
    blocks::BlockStore,
    config::StoreConfig,
    db::migrations::apply_migrations,
    errors::{DatabaseError, DatabaseSetupError, GenesisError, NoteSyncError, StateSyncError},
    genesis::GenesisState,
    types::{AccountId, BlockNumber},
    COMPONENT, SQL_STATEMENT_CACHE_CAPACITY,
};

mod migrations;
mod sql;

mod settings;
#[cfg(test)]
mod tests;

pub type Result<T, E = DatabaseError> = std::result::Result<T, E>;

pub struct Db {
    pool: Pool,
}

#[derive(Debug, PartialEq)]
pub struct NullifierInfo {
    pub nullifier: Nullifier,
    pub block_num: BlockNumber,
}

#[derive(Debug, PartialEq)]
pub struct TransactionSummary {
    pub account_id: AccountId,
    pub block_num: BlockNumber,
    pub transaction_id: TransactionId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteRecord {
    pub block_num: BlockNumber,
    pub note_index: BlockNoteIndex,
    pub note_id: RpoDigest,
    pub metadata: NoteMetadata,
    pub details: Option<Vec<u8>>,
    pub merkle_path: MerklePath,
}

impl From<NoteRecord> for NotePb {
    fn from(note: NoteRecord) -> Self {
        Self {
            block_num: note.block_num,
            note_index: note.note_index.leaf_index_value().into(),
            note_id: Some(note.note_id.into()),
            metadata: Some(note.metadata.into()),
            merkle_path: Some(Into::into(&note.merkle_path)),
            details: note.details,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct StateSyncUpdate {
    pub notes: Vec<NoteSyncRecord>,
    pub block_header: BlockHeader,
    pub account_updates: Vec<AccountSummary>,
    pub transactions: Vec<TransactionSummary>,
    pub nullifiers: Vec<NullifierInfo>,
}

#[derive(Debug, PartialEq)]
pub struct NoteSyncUpdate {
    pub notes: Vec<NoteSyncRecord>,
    pub block_header: BlockHeader,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteSyncRecord {
    pub block_num: BlockNumber,
    pub note_index: BlockNoteIndex,
    pub note_id: RpoDigest,
    pub metadata: NoteMetadata,
    pub merkle_path: MerklePath,
}

impl From<NoteSyncRecord> for NoteSyncRecordPb {
    fn from(note: NoteSyncRecord) -> Self {
        Self {
            note_index: note.note_index.leaf_index_value().into(),
            note_id: Some(note.note_id.into()),
            metadata: Some(note.metadata.into()),
            merkle_path: Some(Into::into(&note.merkle_path)),
        }
    }
}

impl From<NoteRecord> for NoteSyncRecord {
    fn from(note: NoteRecord) -> Self {
        Self {
            block_num: note.block_num,
            note_index: note.note_index,
            note_id: note.note_id,
            metadata: note.metadata,
            merkle_path: note.merkle_path,
        }
    }
}

impl Db {
    /// Open a connection to the DB, apply any pending migrations, and ensure that the genesis block
    /// is as expected and present in the database.
    // TODO: This span is logged in a root span, we should connect it to the parent one.
    #[instrument(target = "miden-store", skip_all)]
    pub async fn setup(
        config: StoreConfig,
        block_store: Arc<BlockStore>,
    ) -> Result<Self, DatabaseSetupError> {
        info!(target: COMPONENT, %config, "Connecting to the database");

        if let Some(p) = config.database_filepath.parent() {
            create_dir_all(p).map_err(DatabaseError::IoError)?;
        }

        let pool = SqliteConfig::new(config.database_filepath.clone())
            .builder(Runtime::Tokio1)
            .expect("Infallible")
            .post_create(Hook::async_fn(move |conn, _| {
                Box::pin(async move {
                    let _ = conn
                        .interact(|conn| {
                            // Feature used to support `IN` and `NOT IN` queries. We need to load
                            // this module for every connection we create to the DB to support the
                            // queries we want to run
                            array::load_module(conn)?;

                            // Increase the statement cache size.
                            conn.set_prepared_statement_cache_capacity(
                                SQL_STATEMENT_CACHE_CAPACITY,
                            );

                            // Enable the WAL mode. This allows concurrent reads while the
                            // transaction is being written, this is required for proper
                            // synchronization of the servers in-memory and on-disk representations
                            // (see [State::apply_block])
                            conn.execute("PRAGMA journal_mode = WAL;", ())?;

                            // Enable foreign key checks.
                            conn.execute("PRAGMA foreign_keys = ON;", ())
                        })
                        .await
                        .map_err(|e| {
                            HookError::Message(format!("Loading carray module failed: {e}").into())
                        })?;

                    Ok(())
                })
            }))
            .build()?;

        info!(
            target: COMPONENT,
            sqlite = format!("{}", config.database_filepath.display()),
            "Connected to the database"
        );

        let conn = pool.get().await.map_err(DatabaseError::MissingDbConnection)?;

        conn.interact(apply_migrations).await.map_err(|err| {
            DatabaseError::InteractError(format!("Migration task failed: {err}"))
        })??;

        let db = Db { pool };
        db.ensure_genesis_block(&config.genesis_filepath.as_path().to_string_lossy(), block_store)
            .await?;

        Ok(db)
    }

    /// Loads all the nullifiers from the DB.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_all_nullifiers(&self) -> Result<Vec<(Nullifier, BlockNumber)>> {
        self.pool
            .get()
            .await?
            .interact(sql::select_all_nullifiers)
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!("Select nullifiers task failed: {err}"))
            })?
    }

    /// Loads the nullifiers that match the prefixes from the DB.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_nullifiers_by_prefix(
        &self,
        prefix_len: u32,
        nullifier_prefixes: Vec<u32>,
    ) -> Result<Vec<NullifierInfo>> {
        self.pool
            .get()
            .await?
            .interact(move |conn| {
                sql::select_nullifiers_by_prefix(conn, prefix_len, &nullifier_prefixes)
            })
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!(
                    "Select nullifiers by prefix task failed: {err}"
                ))
            })?
    }

    /// Loads all the notes from the DB.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_all_notes(&self) -> Result<Vec<NoteRecord>> {
        self.pool.get().await?.interact(sql::select_all_notes).await.map_err(|err| {
            DatabaseError::InteractError(format!("Select notes task failed: {err}"))
        })?
    }

    /// Loads all the accounts from the DB.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_all_accounts(&self) -> Result<Vec<AccountInfo>> {
        self.pool.get().await?.interact(sql::select_all_accounts).await.map_err(|err| {
            DatabaseError::InteractError(format!("Select accounts task failed: {err}"))
        })?
    }

    /// Search for a [BlockHeader] from the database by its `block_num`.
    ///
    /// When `block_number` is [None], the latest block header is returned.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_block_header_by_block_num(
        &self,
        block_number: Option<BlockNumber>,
    ) -> Result<Option<BlockHeader>> {
        self.pool
            .get()
            .await?
            .interact(move |conn| sql::select_block_header_by_block_num(conn, block_number))
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!("Select block header task failed: {err}"))
            })?
    }

    /// Loads multiple block headers from the DB.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_block_headers(&self, blocks: Vec<BlockNumber>) -> Result<Vec<BlockHeader>> {
        self.pool
            .get()
            .await?
            .interact(move |conn| sql::select_block_headers(conn, blocks))
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!(
                    "Select many block headers task failed: {err}"
                ))
            })?
    }

    /// Loads all the block headers from the DB.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_all_block_headers(&self) -> Result<Vec<BlockHeader>> {
        self.pool
            .get()
            .await?
            .interact(sql::select_all_block_headers)
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!("Select block headers task failed: {err}"))
            })?
    }

    /// Loads all the account hashes from the DB.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_all_account_hashes(&self) -> Result<Vec<(AccountId, RpoDigest)>> {
        self.pool
            .get()
            .await?
            .interact(sql::select_all_account_hashes)
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!("Select account hashes task failed: {err}"))
            })?
    }

    /// Loads public account details from the DB.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_account(&self, id: AccountId) -> Result<AccountInfo> {
        self.pool
            .get()
            .await?
            .interact(move |conn| sql::select_account(conn, id))
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!("Get account details task failed: {err}"))
            })?
    }

    /// Loads public accounts details from the DB.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_accounts_by_ids(
        &self,
        account_ids: Vec<AccountId>,
    ) -> Result<Vec<AccountInfo>> {
        self.pool
            .get()
            .await?
            .interact(move |conn| sql::select_accounts_by_ids(conn, &account_ids))
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!("Get accounts details task failed: {err}"))
            })?
    }

    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn get_state_sync(
        &self,
        block_num: BlockNumber,
        account_ids: Vec<AccountId>,
        note_tags: Vec<u32>,
        nullifier_prefixes: Vec<u32>,
    ) -> Result<StateSyncUpdate, StateSyncError> {
        self.pool
            .get()
            .await
            .map_err(DatabaseError::MissingDbConnection)?
            .interact(move |conn| {
                sql::get_state_sync(conn, block_num, &account_ids, &note_tags, &nullifier_prefixes)
            })
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!("Get state sync task failed: {err}"))
            })?
    }

    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn get_note_sync(
        &self,
        block_num: BlockNumber,
        note_tags: Vec<u32>,
    ) -> Result<NoteSyncUpdate, NoteSyncError> {
        self.pool
            .get()
            .await
            .map_err(DatabaseError::MissingDbConnection)?
            .interact(move |conn| sql::get_note_sync(conn, block_num, &note_tags))
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!("Get notes sync task failed: {err}"))
            })?
    }

    /// Loads all the Note's matching a certain NoteId from the database.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_notes_by_id(&self, note_ids: Vec<NoteId>) -> Result<Vec<NoteRecord>> {
        self.pool
            .get()
            .await?
            .interact(move |conn| sql::select_notes_by_id(conn, &note_ids))
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!("Select note by id task failed: {err}"))
            })?
    }

    /// Loads inclusion proofs for notes matching the given IDs.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_note_inclusion_proofs(
        &self,
        note_ids: BTreeSet<NoteId>,
    ) -> Result<BTreeMap<NoteId, NoteInclusionProof>> {
        self.pool
            .get()
            .await?
            .interact(move |conn| sql::select_note_inclusion_proofs(conn, note_ids))
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!(
                    "Select block note inclusion proofs task failed: {err}"
                ))
            })?
    }

    /// Loads all note IDs matching a certain NoteId from the database.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn select_note_ids(&self, note_ids: Vec<NoteId>) -> Result<BTreeSet<NoteId>> {
        self.select_notes_by_id(note_ids)
            .await
            .map(|notes| notes.into_iter().map(|note| note.note_id.into()).collect())
    }

    /// Inserts the data of a new block into the DB.
    ///
    /// `allow_acquire` and `acquire_done` are used to synchronize writes to the DB with writes to
    /// the in-memory trees. Further details available on [super::state::State::apply_block].
    // TODO: This span is logged in a root span, we should connect it to the parent one.
    #[instrument(target = "miden-store", skip_all, err)]
    pub async fn apply_block(
        &self,
        allow_acquire: oneshot::Sender<()>,
        acquire_done: oneshot::Receiver<()>,
        block: Block,
        notes: Vec<NoteRecord>,
    ) -> Result<()> {
        self.pool
            .get()
            .await?
            .interact(move |conn| -> Result<()> {
                // TODO: This span is logged in a root span, we should connect it to the parent one.
                let _span = info_span!(target: COMPONENT, "write_block_to_db").entered();

                let transaction = conn.transaction()?;
                sql::apply_block(
                    &transaction,
                    &block.header(),
                    &notes,
                    block.nullifiers(),
                    block.updated_accounts(),
                )?;

                let _ = allow_acquire.send(());
                acquire_done.blocking_recv()?;

                transaction.commit()?;

                Ok(())
            })
            .await
            .map_err(|err| {
                DatabaseError::InteractError(format!("Apply block task failed: {err}"))
            })??;

        Ok(())
    }

    /// Loads account deltas from the DB for given account ID and block range.
    /// Note, that `from_block` is exclusive and `to_block` is inclusive.
    pub(crate) async fn select_account_state_deltas(
        &self,
        account_id: AccountId,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Result<Vec<AccountDelta>> {
        self.pool
            .get()
            .await
            .map_err(DatabaseError::MissingDbConnection)?
            .interact(move |conn| -> Result<Vec<AccountDelta>> {
                sql::select_account_deltas(conn, account_id, from_block, to_block)
            })
            .await
            .map_err(|err| DatabaseError::InteractError(err.to_string()))?
    }

    // HELPERS
    // ---------------------------------------------------------------------------------------------

    /// If the database is empty, generates and stores the genesis block. Otherwise, it ensures that
    /// the genesis block in the database is consistent with the genesis block data in the
    /// genesis JSON file.
    #[instrument(target = "miden-store", skip_all, err)]
    async fn ensure_genesis_block(
        &self,
        genesis_filepath: &str,
        block_store: Arc<BlockStore>,
    ) -> Result<(), GenesisError> {
        let genesis_block = {
            let file_contents = fs::read(genesis_filepath).map_err(|error| {
                GenesisError::FailedToReadGenesisFile {
                    genesis_filepath: genesis_filepath.to_string(),
                    error,
                }
            })?;

            let genesis_state = GenesisState::read_from_bytes(&file_contents)
                .map_err(GenesisError::GenesisFileDeserializationError)?;

            genesis_state.into_block()?
        };

        let maybe_block_header_in_store = self
            .select_block_header_by_block_num(Some(GENESIS_BLOCK))
            .await
            .map_err(|err| GenesisError::SelectBlockHeaderByBlockNumError(err.into()))?;

        let expected_genesis_header = genesis_block.header();

        match maybe_block_header_in_store {
            Some(block_header_in_store) => {
                // ensure that expected header is what's also in the store
                if expected_genesis_header != block_header_in_store {
                    Err(GenesisError::GenesisBlockHeaderMismatch {
                        expected_genesis_header: Box::new(expected_genesis_header),
                        block_header_in_store: Box::new(block_header_in_store),
                    })?;
                }
            },
            None => {
                // add genesis header to store
                self.pool
                    .get()
                    .await
                    .map_err(DatabaseError::MissingDbConnection)?
                    .interact(move |conn| -> Result<()> {
                        // TODO: This span is logged in a root span, we should connect it to the
                        // parent one.
                        let span = info_span!(target: COMPONENT, "write_genesis_block_to_db");
                        let guard = span.enter();

                        let transaction = conn.transaction()?;
                        sql::apply_block(
                            &transaction,
                            &expected_genesis_header,
                            &[],
                            &[],
                            genesis_block.updated_accounts(),
                        )?;

                        block_store.save_block_blocking(0, &genesis_block.to_bytes())?;

                        transaction.commit()?;

                        drop(guard);
                        Ok(())
                    })
                    .await
                    .map_err(|err| GenesisError::ApplyBlockFailed(err.to_string()))??;
            },
        }

        Ok(())
    }
}
