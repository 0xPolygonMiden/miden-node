use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};

use miden_crypto::{
    hash::rpo::RpoDigest,
    merkle::{EmptySubtreeRoots, MerklePath, NodeMutation},
};
use miden_objects::{
    block::BlockNumber,
    crypto::merkle::{LeafIndex, MutationSet, NodeIndex, SimpleSmt, ValuePath},
    utils::{Deserializable, Serializable},
    Word, ACCOUNT_TREE_DEPTH,
};
use tracing::{info, instrument};

use crate::{
    errors::{DatabaseError, StateInitializationError},
    COMPONENT,
};

type Update = MutationSet<ACCOUNT_TREE_DEPTH, LeafIndex<ACCOUNT_TREE_DEPTH>, Word>;

/// Account SMT with storage of reverse updates for recent blocks
#[derive(Debug)]
pub struct AccountTree<S: PersistentUpdatesStorage> {
    accounts: SimpleSmt<ACCOUNT_TREE_DEPTH>,
    updates: AccountSmtUpdates<S>,
}

impl<S: PersistentUpdatesStorage + Debug> AccountTree<S> {
    #[instrument(target = COMPONENT)]
    pub async fn new(
        initial_accounts: SimpleSmt<ACCOUNT_TREE_DEPTH>,
        updates_storage: S,
        latest_block_num: BlockNumber,
    ) -> Result<Self, StateInitializationError> {
        Ok(Self {
            accounts: initial_accounts,
            updates: AccountSmtUpdates::new(updates_storage, latest_block_num).await?,
        })
    }

    pub fn accounts(&self) -> &SimpleSmt<ACCOUNT_TREE_DEPTH> {
        &self.accounts
    }

    pub async fn apply_mutations(
        &mut self,
        block_num: BlockNumber,
        mutation_set: Update,
    ) -> Result<(), DatabaseError> {
        let reverse_update = self.accounts.apply_mutations_with_reversion(mutation_set)?;
        self.updates.add(block_num, reverse_update).await
    }

    /// Computes opening for the given account state for the specified block number.
    /// It traverses over updated nodes in reverse updates from the root down to the leaf, filling
    /// items in Merkle path, from the given block up to the most recent update, if needed. If there
    /// is no updates left, remaining nodes are got from the current accounts SMT.   
    #[cfg_attr(not(any(test, feature = "bench")), expect(dead_code))]
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
                Self(vec![RpoDigest::default(); ACCOUNT_TREE_DEPTH as usize])
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

        if block_num > self.updates.latest_block_num()
            || block_num
                < self
                    .updates
                    .latest_block_num()
                    .checked_sub(self.updates.num_updates() as u32)
                    .expect("Number of updates can't exceed latest block number")
        {
            return Err(DatabaseError::BlockNotFoundInDb(block_num));
        }

        let leaf_index = LeafIndex::<ACCOUNT_TREE_DEPTH>::new(account_id_prefix)
            .expect("`ACCOUNT_TREE_DEPTH` must not be less than `SMT_MIN_DEPTH`");
        let Some(mut update_index) = self.updates.update_index(block_num) else {
            return Ok(self.accounts.open(&leaf_index));
        };

        let mut node_index = NodeIndex::root();
        let mut path = MutableMerklePath::new();

        // Begin filling path from reverse updates, starting from the root of update for the given
        // block number, moving down through the way to the account's leaf. If a subtree wasn't
        // found (i.e. no leaves were updated), go to the next reverse update.
        while node_index.depth() < ACCOUNT_TREE_DEPTH {
            match self.updates.get(update_index).node_mutations().get(&node_index) {
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
            let Some(leaf) = self.updates.get(update_index).new_pairs().get(&leaf_index) else {
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

    fn is_in_left_subtree(account_id_prefix: u64, from_node_index: NodeIndex) -> bool {
        debug_assert!(from_node_index.depth() < ACCOUNT_TREE_DEPTH);

        account_id_prefix
            < ((from_node_index.value() << 1) + 1)
                * (1 << (ACCOUNT_TREE_DEPTH - from_node_index.depth() - 1))
    }
}

impl Default for AccountTree<()> {
    fn default() -> Self {
        Self {
            accounts: SimpleSmt::new()
                .expect("`DEPTH` must be within the range `SMT_MIN_DEPTH..=SMT_MAX_DEPTH`"),
            updates: AccountSmtUpdates {
                latest_block_num: BlockNumber::GENESIS,
                updates: vec![],
                storage: (),
            },
        }
    }
}

#[derive(Debug)]
pub struct AccountSmtUpdates<S: PersistentUpdatesStorage> {
    latest_block_num: BlockNumber,
    updates: Vec<Update>,
    storage: S,
}

impl<S: PersistentUpdatesStorage + Debug> AccountSmtUpdates<S> {
    /// How many latest block states will be tracked.
    const UPDATES_DEPTH: usize = 99;

    #[instrument(target = COMPONENT, skip(storage))]
    async fn new(storage: S, latest_block_num: BlockNumber) -> Result<Self, DatabaseError> {
        let updates = Self::load_updates(&storage, latest_block_num).await?;

        Ok(Self { latest_block_num, updates, storage })
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
        assert_eq!(block_num, self.latest_block_num.child(), "Block numbers must be contiguous");

        self.storage
            .save(block_num.parent().expect("Latest block number must be positive"), &update)
            .await?;
        self.updates.insert(0, update);
        self.latest_block_num = block_num;
        self.truncate_updates().await?;

        Ok(())
    }

    #[instrument(target = COMPONENT)]
    async fn load_updates(
        storage: &S,
        latest_block_num: BlockNumber,
    ) -> Result<Vec<Update>, DatabaseError> {
        let mut updates = Vec::with_capacity(Self::UPDATES_DEPTH);
        for block_num in (0..latest_block_num.as_u32()).rev() {
            match storage.load(block_num.into()).await? {
                Some(update) => updates.push(update),
                None => break,
            }
        }

        info!(target: COMPONENT, num_updates = updates.len(), "Loaded accounts SMT updates");

        Ok(updates)
    }

    async fn truncate_updates(&mut self) -> Result<(), DatabaseError> {
        while self.updates.len() > Self::UPDATES_DEPTH {
            self.storage
                .remove(
                    self.latest_block_num
                        .checked_sub(self.updates.len() as u32)
                        .expect("Updates number can't exceed latest block number"),
                )
                .await?;
            self.updates.truncate(self.updates.len() - 1);
        }

        Ok(())
    }

    fn update_index(&self, block_num: BlockNumber) -> Option<usize> {
        if block_num >= self.latest_block_num {
            return None;
        }

        Some(self.latest_block_num.as_usize() - block_num.as_usize() - 1)
    }
}

#[expect(async_fn_in_trait)]
pub trait PersistentUpdatesStorage {
    async fn load(&self, block_num: BlockNumber) -> Result<Option<Update>, DatabaseError>;
    async fn save(&self, block_num: BlockNumber, update: &Update) -> Result<(), DatabaseError>;
    async fn remove(&self, block_num: BlockNumber) -> Result<(), DatabaseError>;
}

impl PersistentUpdatesStorage for () {
    async fn load(&self, _block_num: BlockNumber) -> Result<Option<Update>, DatabaseError> {
        Ok(None)
    }

    async fn save(&self, _block_num: BlockNumber, _update: &Update) -> Result<(), DatabaseError> {
        Ok(())
    }

    async fn remove(&self, _block_num: BlockNumber) -> Result<(), DatabaseError> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct FileUpdatesStorage {
    path: PathBuf,
}

impl FileUpdatesStorage {
    pub async fn new(path: PathBuf) -> Result<Self, DatabaseError> {
        tokio::fs::create_dir_all(&path).await?;

        Ok(Self { path })
    }

    fn item_path(path: impl AsRef<Path>, block_num: BlockNumber) -> PathBuf {
        path.as_ref().join(format!("update_{:08x}.dat", block_num.as_u32()))
    }
}

impl PersistentUpdatesStorage for FileUpdatesStorage {
    async fn load(&self, block_num: BlockNumber) -> Result<Option<Update>, DatabaseError> {
        let file_path = Self::item_path(&self.path, block_num);
        match tokio::fs::read(file_path).await {
            Ok(bytes) => Ok(Some(Update::read_from_bytes(&bytes)?)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    async fn save(&self, block_num: BlockNumber, update: &Update) -> Result<(), DatabaseError> {
        let file_path = Self::item_path(&self.path, block_num);
        tokio::fs::write(file_path, &update.to_bytes()).await.map_err(Into::into)
    }

    async fn remove(&self, block_num: BlockNumber) -> Result<(), DatabaseError> {
        let file_path = Self::item_path(&self.path, block_num);
        tokio::fs::remove_file(file_path).await.map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use miden_crypto::{merkle::LeafIndex, Felt, Word};
    use miden_objects::{block::BlockNumber, FieldElement, ACCOUNT_TREE_DEPTH};

    use super::AccountTree;

    #[tokio::test]
    async fn compute_opening() {
        fn account_smt_update(key: u64, value: u64) -> (LeafIndex<ACCOUNT_TREE_DEPTH>, Word) {
            (
                LeafIndex::new(key).unwrap(),
                [Felt::ZERO, Felt::ZERO, Felt::ZERO, Felt::new(value)],
            )
        }

        let ids = [1, 2, 30, 400, 5000, 60000, 700_000, 80_000_000];
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
                account_smt_update(123_432, 9898),
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
                account_smt_update(234_442, 123),
                account_smt_update(ids[6], 1),
                account_smt_update(ids[7], 2),
            ],
            vec![
                account_smt_update(ids[1], 34),
                account_smt_update(ids[2], 9),
                account_smt_update(539_234, 949),
                account_smt_update(241_223, 343),
                account_smt_update(123_132, 123),
            ],
        ];

        let mut tree = AccountTree::default();
        let mut snapshots = vec![];

        for (block_num, update) in updates.into_iter().enumerate() {
            snapshots.push(tree.accounts().clone());
            let mutations = tree.accounts().compute_mutations(update);
            tree.apply_mutations(BlockNumber::from_usize(block_num).child(), mutations)
                .await
                .unwrap();

            for i in 0..=(block_num + 1) {
                for id in ids {
                    let actual = tree.compute_opening(id, BlockNumber::from_usize(i)).unwrap();
                    let expected = if i == snapshots.len() {
                        tree.accounts().open(&LeafIndex::new(id).unwrap())
                    } else {
                        snapshots[i].open(&(LeafIndex::new(id).unwrap()))
                    };

                    assert_eq!(actual, expected);
                }
            }
        }
    }
}
