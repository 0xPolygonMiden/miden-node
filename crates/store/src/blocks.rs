use std::{io::ErrorKind, path::PathBuf};

use miden_objects::block::BlockNumber;

#[derive(Debug)]
pub struct BlockStore {
    store_dir: PathBuf,
}

impl BlockStore {
    pub async fn new(store_dir: PathBuf) -> Result<Self, std::io::Error> {
        tokio::fs::create_dir_all(&store_dir).await?;

        Ok(Self { store_dir })
    }

    pub async fn load_block(
        &self,
        block_num: BlockNumber,
    ) -> Result<Option<Vec<u8>>, std::io::Error> {
        match tokio::fs::read(self.block_path(block_num)).await {
            Ok(data) => Ok(Some(data)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub async fn save_block(
        &self,
        block_num: BlockNumber,
        data: &[u8],
    ) -> Result<(), std::io::Error> {
        let (epoch_path, block_path) = self.epoch_block_path(block_num)?;
        if !epoch_path.exists() {
            tokio::fs::create_dir_all(epoch_path).await?;
        }

        tokio::fs::write(block_path, data).await
    }

    pub fn save_block_blocking(
        &self,
        block_num: BlockNumber,
        data: &[u8],
    ) -> Result<(), std::io::Error> {
        let (epoch_path, block_path) = self.epoch_block_path(block_num)?;
        if !epoch_path.exists() {
            std::fs::create_dir_all(epoch_path)?;
        }

        std::fs::write(block_path, data)
    }

    // HELPER FUNCTIONS
    // --------------------------------------------------------------------------------------------

    fn block_path(&self, block_num: BlockNumber) -> PathBuf {
        let block_num = block_num.as_u32();
        let epoch = block_num >> 16;
        let epoch_dir = self.store_dir.join(format!("{epoch:04x}"));
        epoch_dir.join(format!("block_{block_num:08x}.dat"))
    }

    fn epoch_block_path(
        &self,
        block_num: BlockNumber,
    ) -> Result<(PathBuf, PathBuf), std::io::Error> {
        let block_path = self.block_path(block_num);
        let epoch_path = block_path.parent().ok_or(std::io::Error::from(ErrorKind::NotFound))?;

        Ok((epoch_path.to_path_buf(), block_path))
    }
}
