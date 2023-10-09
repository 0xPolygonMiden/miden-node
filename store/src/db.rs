use crate::{config::StoreConfig, migrations, types::BlockNumber};
use anyhow::anyhow;
use deadpool_sqlite::{rusqlite::types::ValueRef, Config as SqliteConfig, Pool, Runtime};
use miden_crypto::{
    hash::rpo::RpoDigest,
    utils::{Deserializable, SliceReader},
};
use miden_node_proto::block_header::BlockHeader;
use prost::Message;
use std::fs::create_dir_all;
use tracing::info;

pub struct Db {
    pool: Pool,
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

        conn.interact(|conn| migrations::MIGRATIONS.to_latest(conn))
            .await
            .map_err(|_| anyhow!("Migration task failed with a panic"))??;

        Ok(Db { pool })
    }

    pub async fn get_nullifiers(&self) -> Result<Vec<(RpoDigest, BlockNumber)>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(|conn| {
                let mut stmt = conn.prepare("SELECT nullifier, block_number FROM nullifiers;")?;
                let mut rows = stmt.query([])?;
                let mut result = vec![];
                while let Some(row) = rows.next()? {
                    let nullifier = match row.get_ref_unwrap(0) {
                        ValueRef::Blob(data) => {
                            let mut reader = SliceReader::new(data);
                            RpoDigest::read_from(&mut reader)
                                .map_err(|_| anyhow!("Decoding nullifier from DB failed"))?
                        },
                        _ => unreachable!(),
                    };
                    let block_number = row.get(1)?;
                    result.push((nullifier, block_number));
                }

                Ok(result)
            })
            .await
            .map_err(|_| anyhow!("Get nullifiers task failed with a panic"))?
    }

    pub async fn get_block_header(
        &self,
        block_number: Option<u64>,
    ) -> Result<Option<BlockHeader>, anyhow::Error> {
        self.pool
            .get()
            .await?
            .interact(move |conn| {
                let mut stmt;
                let mut rows = match block_number {
                    Some(block_number) => {
                        stmt = conn.prepare("SELECT block_header FROM block_header WHERE block_number = ?1")?;
                        stmt.query([block_number])?
                    },
                    None => {
                        stmt = conn.prepare("SELECT block_header FROM block_header ORDER BY block_number DESC LIMIT 1")?;
                        stmt.query([])?
                    },
                };

                match rows.next()? {
                    Some(row) =>  {
                        let data = row.get_ref_unwrap(0).as_blob()?;
                        Ok(Some(BlockHeader::decode(data)?))
                    },
                    None => Ok(None),
                }
            })
            .await
            .map_err(|_| anyhow!("Get block header task failed with a panic"))?
    }
}
