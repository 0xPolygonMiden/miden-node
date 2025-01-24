use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};

use miden_crypto::{
    hash::rpo::RpoDigest,
    merkle::{EmptySubtreeRoots, MerklePath, NodeMutation},
};
use miden_objects::{
    crypto::merkle::{LeafIndex, MutationSet, NodeIndex, SimpleSmt, ValuePath},
    utils::{Deserializable, Serializable},
    Word, ACCOUNT_TREE_DEPTH,
};
use tracing::{info, instrument};

use crate::{
    db::Db,
    errors::{DatabaseError, StateInitializationError},
    types::BlockNumber,
    COMPONENT,
};

type Update = MutationSet<ACCOUNT_TREE_DEPTH, LeafIndex<ACCOUNT_TREE_DEPTH>, Word>;

/// Account SMT with storage of reverse updates for recent blocks
#[derive(Debug)]
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
        tokio::fs::create_dir_all(&storage_path).await.map_err(DatabaseError::IoError)?;

        Ok(Self {
            accounts: Self::load_accounts(db).await?,
            update_storage: UpdateStorage::new(storage_path, latest_block_num).await?,
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

    /// Computes opening for the given account state for the specified block number.
    /// It traverses over updated nodes in reverse updates from the root down to the leaf, filling
    /// items in Merkle path, from the given block up to the most recent update, if needed. If there
    /// is no updates left, remaining nodes are got from the current accounts SMT.   
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn compute_opening(
        &self,
        account_id_prefix: u64,
        block_num: BlockNumber,
    ) -> Result<ValuePath, DatabaseError> {
        /// Structure for filling of mutable Merkle path
        #[derive(Debug)]
        struct MutableMerklePath(Vec<RpoDigest>);

        impl MutableMerklePath {
            fn new() -> Self {
                Self(vec![Default::default(); ACCOUNT_TREE_DEPTH as usize])
            }

            fn update_item(&mut self, depth: u8, value: RpoDigest) {
                debug_assert!(depth > 0);
                debug_assert!(depth <= ACCOUNT_TREE_DEPTH);

                let index = self.0.len() - depth as usize;
                self.0[index] = value;
            }

            fn into_merkle_path(self) -> MerklePath {
                MerklePath::new(self.0)
            }
        }

        if block_num > self.update_storage.latest_block_num()
            || (block_num as usize)
                < self.update_storage.latest_block_num() as usize
                    - self.update_storage.num_updates()
        {
            return Err(DatabaseError::BlockNotFoundInDb(block_num));
        }

        let leaf_index = LeafIndex::<ACCOUNT_TREE_DEPTH>::new(account_id_prefix)
            .expect("`ACCOUNT_TREE_DEPTH` must not be less than `SMT_MIN_DEPTH`");
        let Some(mut update_index) = self.update_storage.update_index(block_num) else {
            return Ok(self.accounts.open(&leaf_index));
        };

        let mut node_index = NodeIndex::root();
        let mut path = MutableMerklePath::new();

        // Begin filling path from reverse updates, starting from the root of update for the given
        // block number, moving down through the way to the account's leaf. If a subtree wasn't
        // found (i.e. no leaves were updated), go to the next reverse update.
        while node_index.depth() < ACCOUNT_TREE_DEPTH {
            match self.update_storage.get(update_index).node_mutations().get(&node_index) {
                None if update_index == 0 => {
                    break;
                },

                None => {
                    update_index -= 1;
                },

                Some(mutation) => {
                    let is_left = Self::is_in_left_subtree(account_id_prefix, node_index);

                    if is_left {
                        node_index = node_index.left_child();
                    } else {
                        node_index = node_index.right_child();
                    }

                    let path_item = match mutation {
                        NodeMutation::Removal => {
                            *EmptySubtreeRoots::entry(ACCOUNT_TREE_DEPTH, node_index.depth())
                        },

                        NodeMutation::Addition(node) => {
                            if is_left {
                                node.right
                            } else {
                                node.left
                            }
                        },
                    };

                    path.update_item(node_index.depth(), path_item);
                },
            }
        }

        // If went down to the leaf it was looking for, construct and return opening using the leaf
        // from the current update.
        if node_index.depth() == ACCOUNT_TREE_DEPTH {
            let Some(leaf) = self.update_storage.get(update_index).new_pairs().get(&leaf_index)
            else {
                return Err(DatabaseError::DataCorrupted(format!(
                    "No new pair for the leaf {leaf_index:?} were found in update #{update_index}"
                )));
            };

            return Ok(ValuePath::new(leaf.into(), path.into_merkle_path()));
        }

        // No updates left, but the path was not yet completed, we need to fill remaining path part
        // from the current account's SMT.
        while node_index.depth() < ACCOUNT_TREE_DEPTH {
            let is_left = Self::is_in_left_subtree(account_id_prefix, node_index);

            let node = self
                .accounts
                .get_node(if is_left {
                    node_index.right_child()
                } else {
                    node_index.left_child()
                })
                .expect("Depth must be in the range 0..=ACCOUNT_TREE_DEPTH");

            if is_left {
                node_index = node_index.left_child();
            } else {
                node_index = node_index.right_child();
            }

            path.update_item(node_index.depth(), node);
        }

        let leaf = self.accounts.get_leaf(&leaf_index);

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

#[derive(Debug)]
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

    pub fn num_updates(&self) -> usize {
        self.updates.len()
    }

    pub fn get(&self, index: usize) -> &Update {
        &self.updates[index]
    }

    pub async fn add(
        &mut self,
        block_num: BlockNumber,
        update: Update,
    ) -> Result<(), DatabaseError> {
        assert!(block_num > 0, "Latest block number must be positive");
        assert_eq!(block_num, self.latest_block_num + 1, "Block numbers must be contiguous");

        self.store_update(Self::item_path(&self.path, block_num - 1), &update).await?;
        self.add_internal(block_num, update);
        self.truncate_updates().await?;

        Ok(())
    }

    fn add_internal(&mut self, block_num: BlockNumber, update: Update) {
        self.updates.insert(0, update);
        self.latest_block_num = block_num;
    }

    #[instrument(target = COMPONENT)]
    async fn load_updates(
        path: impl AsRef<Path> + Debug,
        latest_block_num: BlockNumber,
    ) -> Result<Vec<Update>, DatabaseError> {
        let mut updates = Vec::with_capacity(Self::UPDATES_DEPTH);
        for block_num in (0..latest_block_num).rev() {
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

        info!(target: COMPONENT, num_updates = updates.len(), "Loaded accounts SMT updates");

        Ok(updates)
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
                self.latest_block_num - self.updates.len() as BlockNumber,
            );
            tokio::fs::remove_file(file_path).await?;
            self.updates.truncate(self.updates.len() - 1);
        }

        Ok(())
    }

    fn item_path(path: impl AsRef<Path>, block_num: BlockNumber) -> PathBuf {
        path.as_ref().join(format!("update_{block_num:08x}.dat"))
    }

    fn update_index(&self, block_num: BlockNumber) -> Option<usize> {
        if block_num >= self.latest_block_num {
            return None;
        }

        Some((self.latest_block_num - block_num - 1) as usize)
    }
}

#[cfg(test)]
mod tests {
    use miden_crypto::{
        merkle::{LeafIndex, SimpleSmt},
        Felt, Word,
    };
    use miden_objects::{FieldElement, ACCOUNT_TREE_DEPTH};

    use super::{AccountTree, UpdateStorage};
    use crate::types::BlockNumber;

    #[test]
    fn compute_opening() {
        fn account_smt_update(key: u64, value: u64) -> (LeafIndex<ACCOUNT_TREE_DEPTH>, Word) {
            (
                LeafIndex::new(key).unwrap(),
                [Felt::ZERO, Felt::ZERO, Felt::ZERO, Felt::new(value)],
            )
        }

        let ids = [1_u64, 2, 30, 400, 5000, 60000, 700000, 80000000];
        let updates = [
            vec![
                account_smt_update(ids[0], 1),
                account_smt_update(ids[1], 2),
                account_smt_update(ids[2], 3),
                account_smt_update(ids[3], 4),
                account_smt_update(ids[7], 45),
            ],
            vec![
                account_smt_update(ids[1], 34),
                account_smt_update(ids[2], 352),
                account_smt_update(ids[3], 34),
                account_smt_update(ids[5], 31),
                account_smt_update(ids[7], 4),
                account_smt_update(123432, 9898),
            ],
            vec![
                account_smt_update(ids[3], 134),
                account_smt_update(ids[5], 331),
                account_smt_update(ids[7], 54),
            ],
            vec![
                account_smt_update(ids[0], 4),
                account_smt_update(ids[2], 2),
                account_smt_update(ids[3], 0),
                account_smt_update(234442, 123),
                account_smt_update(ids[6], 1),
                account_smt_update(ids[7], 2),
            ],
            vec![
                account_smt_update(ids[1], 34),
                account_smt_update(ids[2], 9),
                account_smt_update(539234, 949),
                account_smt_update(241223, 343),
                account_smt_update(123132, 123),
            ],
        ];

        let mut tree = AccountTree {
            accounts: SimpleSmt::new().unwrap(),
            update_storage: UpdateStorage {
                path: Default::default(),
                latest_block_num: 0,
                updates: vec![],
            },
        };
        let mut snapshots = vec![];

        for (block_num, update) in updates.into_iter().enumerate() {
            snapshots.push(tree.accounts().clone());
            let mutations = tree.accounts().compute_mutations(update);
            let reverse_update =
                tree.accounts_mut().apply_mutations_with_reversion(mutations).unwrap();
            tree.update_storage_mut()
                .add_internal((block_num + 1) as BlockNumber, reverse_update.clone());

            for i in 0..=(block_num + 1) {
                for id in ids.iter() {
                    let actual = tree.compute_opening(*id, i as BlockNumber).unwrap();
                    let expected = if i == snapshots.len() {
                        tree.accounts().open(&LeafIndex::new(*id).unwrap())
                    } else {
                        snapshots[i].open(&(LeafIndex::new(*id).unwrap()))
                    };

                    assert_eq!(actual, expected);
                }
            }
        }
    }
}
