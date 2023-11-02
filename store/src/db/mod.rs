use crate::{
    config::StoreConfig,
    migrations,
    types::{AccountId, BlockNumber},
};
use anyhow::anyhow;
use deadpool_sqlite::{Config as SqliteConfig, Pool, Runtime};
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
use rusqlite::{params, types::Value, vtab::array};
use std::{fs::create_dir_all, rc::Rc};
use tracing::info;

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

    /// Inserts a new nullifier to the DB.
    ///
    /// This method may be called multiple times with the same nullifier.
    #[cfg(test)]
    pub async fn add_nullifier(
        &self,
        nullifier: RpoDigest,
        block_number: BlockNumber,
    ) -> Result<usize, anyhow::Error> {
        use miden_crypto::StarkField;

        let num_rows = self
            .pool
            .get()
            .await?
            .interact(move |conn| {
                let mut stmt = conn
                    .prepare("INSERT INTO nullifiers (nullifier, nullifier_prefix, block_number) VALUES (?1, ?2, ?3);")?;
                stmt.execute(params![nullifier.as_bytes(), u64_to_prefix(nullifier[0].as_int()), block_number])
            })
            .await
            .map_err(|_| anyhow!("Add nullifier task failed with a panic"))??;

        Ok(num_rows)
    }

    /// Loads all the nullifiers from the DB.
    pub async fn get_nullifiers(&self) -> Result<Vec<(RpoDigest, BlockNumber)>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(|conn| {
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
            })
            .await
            .map_err(|_| anyhow!("Get nullifiers task failed with a panic"))?
    }

    /// Returns nullifiers created in the `(block_start, block_end]` range which also match the
    /// `nullifiers` filter.
    ///
    /// Each value of the `nullifiers` is only the 16 most significat bits of the nullifier of
    /// interest to the client. This hides the details of the specific nullifier being requested.
    pub async fn get_nullifiers_by_block_range(
        &self,
        block_start: BlockNumber,
        block_end: BlockNumber,
        nullifiers: &[u32],
    ) -> Result<Vec<NullifierUpdate>, anyhow::Error> {
        let nullifiers: Vec<_> = nullifiers.iter().copied().map(u32_to_value).collect();

        self.pool
            .get()
            .await?
            .interact(move |conn| {
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
            })
            .await
            .map_err(|_| anyhow!("Get nullifiers by block number task failed with a panic"))?
    }

    /// Save a [BlockHeader] to the DB.
    #[cfg(test)]
    pub async fn add_block_header(
        &self,
        block_header: BlockHeader,
    ) -> Result<usize, anyhow::Error> {
        let num_rows = self
            .pool
            .get()
            .await?
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "INSERT INTO block_headers (block_num, block_header) VALUES (?1, ?2);",
                )?;
                stmt.execute(params![block_header.block_num, block_header.encode_to_vec()])
            })
            .await
            .map_err(|_| anyhow!("Add block header task failed with a panic"))??;

        Ok(num_rows)
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
            .interact(move |conn| {
                let mut stmt;
                let mut rows = match block_number {
                    Some(block_number) => {
                        stmt = conn.prepare("SELECT block_header FROM block_headers WHERE block_num = ?1")?;
                        stmt.query([block_number])?
                    },
                    None => {
                        stmt = conn.prepare("SELECT block_header FROM block_headers ORDER BY block_num DESC LIMIT 1")?;
                        stmt.query([])?
                    },
                };

                match rows.next()? {
                    Some(row) =>  {
                        let data = row.get_ref(0)?.as_blob()?;
                        Ok(Some(BlockHeader::decode(data)?))
                    },
                    None => Ok(None),
                }
            })
            .await
            .map_err(|_| anyhow!("Get block header task failed with a panic"))?
    }

    /// Loads all the block headers from the DB.
    pub async fn get_block_headers(&self) -> Result<Vec<BlockHeader>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(|conn| {
                let mut stmt = conn.prepare("SELECT block_header FROM block_headers;")?;
                let mut rows = stmt.query([])?;
                let mut result = vec![];
                while let Some(row) = rows.next()? {
                    let block_header_data = row.get_ref(0)?.as_blob()?;
                    let block_header = BlockHeader::decode(block_header_data)?;
                    result.push(block_header);
                }

                Ok(result)
            })
            .await
            .map_err(|_| anyhow!("Get block headers task failed with a panic"))?
    }

    /// Save a [Note] to the DB.
    #[cfg(test)]
    pub async fn add_note(
        &self,
        note: Note,
    ) -> Result<usize, anyhow::Error> {
        let num_rows = self
            .pool
            .get()
            .await?
            .interact(move |conn| -> Result<_, anyhow::Error> {
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
            })
            .await
            .map_err(|_| anyhow!("Add note task failed with a panic"))??;

        Ok(num_rows)
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
    pub async fn get_notes_since_block_by_tag_and_sender(
        &self,
        tags: &[u32],
        account_ids: &[u64],
        block_num: BlockNumber,
    ) -> Result<Vec<Note>, anyhow::Error> {
        let tags: Vec<Value> = tags.iter().copied().map(u32_to_value).collect();
        let account_ids = account_ids
            .iter()
            .copied()
            .map(u64_to_value)
            .collect::<Result<Vec<Value>, anyhow::Error>>()?;

        let notes = self
            .pool
            .get()
            .await?
            .interact(move |conn| -> Result<_, anyhow::Error> {
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
                let mut rows =
                    stmt.query(params![Rc::new(tags), Rc::new(account_ids), block_num])?;

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
            })
            .await
            .map_err(|_| anyhow!("Get notes since block by tag task failed with a panic"))??;

        Ok(notes)
    }

    /// Inserts or updates an account's hash in the DB.
    #[cfg(test)]
    pub async fn update_account_hash(
        &self,
        account_id: AccountId,
        account_hash: Digest,
        block_num: BlockNumber,
    ) -> Result<usize, anyhow::Error> {
        let num_rows = self
            .pool
            .get()
            .await?
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "INSERT OR REPLACE INTO accounts (account_id, account_hash, block_num) VALUES (?1, ?2, ?3);",
                )?;
                stmt.execute(params![account_id, account_hash.encode_to_vec(), block_num])
            })
            .await
            .map_err(|_| anyhow!("Update account hash task failed with a panic"))??;

        Ok(num_rows)
    }

    // Returns the account hash of the ones that have changed in the range `(block_start, block_end]`.
    pub async fn get_account_hash_by_block_range(
        &self,
        block_start: BlockNumber,
        block_end: BlockNumber,
        account_ids: &[AccountId],
    ) -> Result<Vec<AccountHashUpdate>, anyhow::Error> {
        let account_ids = account_ids
            .iter()
            .copied()
            .map(u64_to_value)
            .collect::<Result<Vec<Value>, anyhow::Error>>()?;

        self.pool
            .get()
            .await?
            .interact(move |conn| {
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
            })
            .await
            .map_err(|_| anyhow!("Get account hash by block number task failed with a panic"))?
    }

    /// Loads all the account hashes from the DB.
    pub async fn get_account_hashes(&self) -> Result<Vec<(AccountId, Digest)>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(move |conn| {
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
            })
            .await
            .map_err(|_| anyhow!("Get account hashes task failed with a panic"))?
    }

    pub async fn get_state_sync(
        &self,
        block_num: BlockNumber,
        account_ids: &[u64],
        note_tag_prefixes: &[u32],
        nullifiers_prefix: &[u32],
    ) -> Result<StateSyncUpdate, anyhow::Error> {
        let notes = self
            .get_notes_since_block_by_tag_and_sender(&note_tag_prefixes, &account_ids, block_num)
            .await?;

        let (block_header, chain_tip) = if !notes.is_empty() {
            let block_header = self
                .get_block_header(Some(notes[0].block_num))
                .await?
                .ok_or(anyhow!("Block db is empty"))?;
            let tip = self.get_block_header(None).await?.ok_or(anyhow!("Block db is empty"))?;

            (block_header, tip.block_num)
        } else {
            let block_header =
                self.get_block_header(None).await?.ok_or(anyhow!("Block db is empty"))?;

            let block_num = block_header.block_num;
            (block_header, block_num)
        };

        let account_updates = self
            .get_account_hash_by_block_range(block_num, block_header.block_num, &account_ids)
            .await?;

        let nullifiers = self
            .get_nullifiers_by_block_range(block_num, block_header.block_num, &nullifiers_prefix)
            .await?;

        Ok(StateSyncUpdate {
            notes,
            block_header,
            chain_tip,
            account_updates,
            nullifiers,
        })
    }
}

// UTILITIES
// ================================================================================================

/// Decodes a blob from the database into a [RpoDigest].
fn decode_rpo_digest(data: &[u8]) -> Result<RpoDigest, anyhow::Error> {
    let mut reader = SliceReader::new(data);
    RpoDigest::read_from(&mut reader).map_err(|_| anyhow!("Decoding nullifier from DB failed"))
}

/// Decodes a blob from the database into a [Digest].
fn decode_protobuf_digest(data: &[u8]) -> Result<Digest, anyhow::Error> {
    Ok(Digest::decode(data)?)
}

/// Converts a `u64` into a [Value].
///
/// Sqlite uses `i64` as its internal representation format.
fn u64_to_value(v: u64) -> Result<Value, anyhow::Error> {
    let v: i64 = v.try_into()?;
    Ok(Value::Integer(v))
}

/// Converts a `u32` into a [Value].
///
/// Sqlite uses `i64` as its internal representation format.
fn u32_to_value(v: u32) -> Value {
    let v: i64 = v.into();
    Value::Integer(v)
}

/// Returns the high bits of the `u64` value used during searches.
fn u64_to_prefix(v: u64) -> u32 {
    (v >> 48) as u32
}
