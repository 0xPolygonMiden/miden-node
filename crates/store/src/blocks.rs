use std::{future::Future, path::PathBuf};

pub trait BlockStorage: Send + Sync + 'static {
    fn save_block(
        &self,
        block_num: u32,
        data: &[u8],
    ) -> impl Future<Output = Result<(), std::io::Error>> + Send;

    fn load_block(
        &self,
        block_num: u32,
    ) -> impl Future<Output = Result<Option<Vec<u8>>, std::io::Error>> + Send;
}

#[derive(Debug)]
pub struct BlockStorageDefault {
    blocks_dir: PathBuf,
}

impl BlockStorageDefault {
    pub async fn new(blocks_dir: PathBuf) -> Result<Self, std::io::Error> {
        tokio::fs::create_dir_all(&blocks_dir).await?;

        Ok(Self { blocks_dir })
    }

    fn block_path(&self, block_num: u32) -> PathBuf {
        self.blocks_dir.join(format!("{block_num:08x}.bin"))
    }
}

impl BlockStorage for BlockStorageDefault {
    async fn save_block(&self, block_num: u32, data: &[u8]) -> Result<(), std::io::Error> {
        tokio::fs::write(self.block_path(block_num), data).await
    }

    async fn load_block(&self, block_num: u32) -> Result<Option<Vec<u8>>, std::io::Error> {
        match tokio::fs::read(self.block_path(block_num)).await {
            Ok(data) => Ok(Some(data)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }
}
