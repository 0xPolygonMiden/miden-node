use std::path::{Path, PathBuf};

use miden_crypto::{
    hash::rpo::RpoDigest,
    merkle::{MerklePath, NodeMutation},
};
use miden_objects::{
    accounts::AccountId,
    crypto::merkle::{LeafIndex, MutationSet, NodeIndex, SimpleSmt, ValuePath},
    utils::{Deserializable, Serializable},
    Word, ACCOUNT_TREE_DEPTH,
};
use tracing::instrument;

use crate::{
    db::Db,
    errors::{DatabaseError, StateInitializationError},
    types::BlockNumber,
    COMPONENT,
};

type Update = MutationSet<ACCOUNT_TREE_DEPTH, LeafIndex<ACCOUNT_TREE_DEPTH>, Word>;

pub struct AccountTree {
    accounts: SimpleSmt<ACCOUNT_TREE_DEPTH>,
    update_storage: UpdateStorage,
}

impl AccountTree {
    #[instrument(target = COMPONENT, skip(db))]
    pub async fn new(
        db: &mut Db,
        storage_path: PathBuf,
        latest_block_num: BlockNumber,
    ) -> Result<Self, StateInitializationError> {
        Ok(Self {
            accounts: Self::load_accounts(db).await?,
            update_storage: UpdateStorage::new(storage_path, latest_block_num - 1).await?,
        })
    }

    pub fn accounts(&self) -> &SimpleSmt<ACCOUNT_TREE_DEPTH> {
        &self.accounts
    }

    pub fn accounts_mut(&mut self) -> &mut SimpleSmt<ACCOUNT_TREE_DEPTH> {
        &mut self.accounts
    }

    pub fn update_storage_mut(&mut self) -> &mut UpdateStorage {
        &mut self.update_storage
    }

    #[expect(dead_code)]
    pub fn compute_opening(
        &self,
        account_id: &AccountId,
        block_num: BlockNumber,
    ) -> Result<ValuePath, DatabaseError> {
        struct MutableMerklePath(Vec<RpoDigest>);

        impl MutableMerklePath {
            fn new(root: RpoDigest) -> Self {
                Self(vec![root; ACCOUNT_TREE_DEPTH as usize])
            }

            fn update_item(&mut self, depth: u8, value: RpoDigest) {
                assert!(depth < ACCOUNT_TREE_DEPTH);

                let index = self.0.len() - depth as usize - 1;
                self.0[index] = value;
            }

            fn into_merkle_path(self) -> MerklePath {
                MerklePath::new(self.0)
            }
        }

        if block_num != self.update_storage.latest_block_num() + 1
            && !self.update_storage.has_update(block_num)
        {
            return Err(DatabaseError::BlockNotFoundInDb(block_num));
        }

        let account_id_prefix = account_id.prefix().into();
        let leaf_index = LeafIndex::<ACCOUNT_TREE_DEPTH>::new(account_id_prefix)
            .expect("`ACCOUNT_TREE_DEPTH` must not be less than `SMT_MIN_DEPTH`");
        let Some(mut update_index) = self.update_storage.update_index(block_num) else {
            return Ok(self.accounts.open(&leaf_index));
        };
        let mut node_index = NodeIndex::root();
        let mut path = MutableMerklePath::new(self.update_storage.get(update_index).root());
        while node_index.depth() < ACCOUNT_TREE_DEPTH {
            match self.update_storage.get(update_index).node_mutations().get(&node_index) {
                None | Some(NodeMutation::Removal) => {
                    if update_index == 0 {
                        break;
                    }
                    update_index -= 1;
                },
                Some(NodeMutation::Addition(node)) => {
                    if Self::is_in_left_subtree(account_id_prefix, node_index) {
                        node_index = node_index.left_child();
                        path.update_item(node_index.depth(), node.left);
                    } else {
                        node_index = node_index.right_child();
                        path.update_item(node_index.depth(), node.right);
                    }
                },
            }
        }

        while node_index.depth() < ACCOUNT_TREE_DEPTH {
            if Self::is_in_left_subtree(account_id_prefix, node_index) {
                node_index = node_index.left_child();
            } else {
                node_index = node_index.right_child();
            }
            if node_index.depth() == ACCOUNT_TREE_DEPTH {
                let leaf = self.accounts.get_leaf(&leaf_index);

                return Ok(ValuePath::new(leaf.into(), path.into_merkle_path()));
            } else {
                let node = self
                    .accounts
                    .get_node(node_index)
                    .expect("Depth must be in range of 0..=ACCOUNT_TREE_DEPTH");
                path.update_item(node_index.depth(), node);
            }
        }

        let Some(leaf) = self.update_storage.get(update_index).new_pairs().get(&leaf_index) else {
            return Err(DatabaseError::DataCorrupted(format!(
                "No new pair for leaf {leaf_index:?} in update #{update_index}"
            )));
        };

        Ok(ValuePath::new(leaf.into(), path.into_merkle_path()))
    }

    #[instrument(target = COMPONENT, skip_all)]
    async fn load_accounts(
        db: &mut Db,
    ) -> Result<SimpleSmt<ACCOUNT_TREE_DEPTH>, StateInitializationError> {
        let account_data: Vec<_> = db
            .select_all_account_hashes()
            .await?
            .into_iter()
            .map(|(id, account_hash)| (id.prefix().into(), account_hash.into()))
            .collect();

        SimpleSmt::with_leaves(account_data)
            .map_err(StateInitializationError::FailedToCreateAccountsTree)
    }

    fn is_in_left_subtree(account_id_prefix: u64, from_node_index: NodeIndex) -> bool {
        debug_assert!(from_node_index.depth() < ACCOUNT_TREE_DEPTH);

        account_id_prefix
            < ((from_node_index.value() << 1) + 1)
                * (1 << (ACCOUNT_TREE_DEPTH - from_node_index.depth() - 1))
    }
}

pub struct UpdateStorage {
    path: PathBuf,
    latest_block_num: BlockNumber,
    updates: Vec<Update>,
}

impl UpdateStorage {
    /// How many latest block states will be tracked.
    const UPDATES_DEPTH: usize = 99;

    #[instrument(target = COMPONENT)]
    async fn new(path: PathBuf, latest_block_num: BlockNumber) -> Result<Self, DatabaseError> {
        let updates = Self::load_updates(&path, latest_block_num).await?;

        Ok(Self { path, latest_block_num, updates })
    }

    pub fn latest_block_num(&self) -> BlockNumber {
        self.latest_block_num
    }

    pub fn has_update(&self, block_num: BlockNumber) -> bool {
        (block_num <= self.latest_block_num)
            && ((self.latest_block_num - block_num) as usize) < self.updates.len()
    }

    pub fn get(&self, index: usize) -> &Update {
        &self.updates[index]
    }

    async fn load_updates(
        path: impl AsRef<Path>,
        latest_block_num: BlockNumber,
    ) -> Result<Vec<Update>, DatabaseError> {
        let mut updates = Vec::with_capacity(Self::UPDATES_DEPTH);
        for block_num in (0..=latest_block_num).rev() {
            match Self::load_update(Self::item_path(&path, block_num)).await {
                Ok(update) => updates.push(update),
                Err(DatabaseError::IoError(err)) => {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        break;
                    }
                    return Err(err.into());
                },
                Err(err) => return Err(err),
            }
        }

        Ok(updates)
    }

    pub async fn add(
        &mut self,
        block_num: BlockNumber,
        update: Update,
    ) -> Result<(), DatabaseError> {
        assert_eq!(block_num, self.latest_block_num + 1, "Block numbers must be contiguous");

        self.store_update(Self::item_path(&self.path, block_num), &update).await?;
        self.updates.insert(0, update);
        self.latest_block_num = block_num;
        self.truncate_updates().await?;

        Ok(())
    }

    async fn store_update(
        &self,
        file_path: impl AsRef<Path>,
        update: &Update,
    ) -> std::io::Result<()> {
        tokio::fs::write(file_path, &update.to_bytes()).await
    }

    async fn load_update(file_path: impl AsRef<Path>) -> Result<Update, DatabaseError> {
        let serialized = tokio::fs::read(file_path).await?;

        Update::read_from_bytes(&serialized).map_err(Into::into)
    }

    async fn truncate_updates(&mut self) -> std::io::Result<()> {
        while self.updates.len() > Self::UPDATES_DEPTH {
            let file_path = Self::item_path(
                &self.path,
                self.latest_block_num + 1 - self.updates.len() as BlockNumber,
            );
            tokio::fs::remove_file(file_path).await?;
            self.updates.truncate(self.updates.len() - 1);
        }

        Ok(())
    }

    fn item_path(path: impl AsRef<Path>, block_num: BlockNumber) -> PathBuf {
        path.as_ref().join(format!("update_{block_num:0x}"))
    }

    fn update_index(&self, block_num: BlockNumber) -> Option<usize> {
        if block_num > self.latest_block_num {
            return None;
        }

        Some((self.latest_block_num - block_num) as usize)
    }
}
