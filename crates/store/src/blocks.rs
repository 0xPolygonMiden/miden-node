use std::{io::ErrorKind, path::PathBuf};

#[derive(Debug)]
pub struct BlockStore {
    store_dir: PathBuf,
}

impl BlockStore {
    pub async fn new(store_dir: PathBuf) -> Result<Self, std::io::Error> {
        tokio::fs::create_dir_all(&store_dir).await?;

        Ok(Self { store_dir })
    }

    pub fn save_block(&self, block_num: u32, data: &[u8]) -> Result<(), std::io::Error> {
        let block_path = self.block_path(block_num);
        let epoch_path = block_path.parent().ok_or(std::io::Error::from(ErrorKind::NotFound))?;

        if !epoch_path.exists() {
            std::fs::create_dir_all(epoch_path)?;
        }

        std::fs::write(block_path, data)
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
        let epoch = block_num >> 16;
        let epoch_dir = self.store_dir.join(format!("{epoch:04x}"));
        epoch_dir.join(format!("block_{block_num:08x}.dat"))
    }
}
