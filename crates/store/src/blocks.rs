use std::path::PathBuf;

#[derive(Debug)]
pub struct BlockStore {
    blockstore_dir: PathBuf,
}

impl BlockStore {
    pub async fn new(blockstore_dir: PathBuf) -> Result<Self, std::io::Error> {
        tokio::fs::create_dir_all(&blockstore_dir).await?;

        Ok(Self { blockstore_dir })
    }

    pub fn save_block(&self, block_num: u32, data: &[u8]) -> Result<(), std::io::Error> {
        std::fs::write(self.block_path(block_num), data)
    }

    pub async fn load_block(&self, block_num: u32) -> Result<Option<Vec<u8>>, std::io::Error> {
        match tokio::fs::read(self.block_path(block_num)).await {
            Ok(data) => Ok(Some(data)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    // HELPER FUNCTIONS
    // --------------------------------------------------------------------------------------------

    fn block_path(&self, block_num: u32) -> PathBuf {
        self.blockstore_dir.join(format!("block_{block_num:08x}.dat"))
    }
}
