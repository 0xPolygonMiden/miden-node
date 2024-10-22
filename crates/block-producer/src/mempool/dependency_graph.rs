use std::collections::{BTreeMap, BTreeSet};

use miden_tx::utils::collections::KvMap;

// DEPENDENCY GRAPH
// ================================================================================================

/// Tracks
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyGraph<K, V> {
    vertices: BTreeMap<K, V>,
    parents: BTreeMap<K, BTreeSet<K>>,
    children: BTreeMap<K, BTreeSet<K>>,
    roots: BTreeSet<K>,
    completed: BTreeSet<K>,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum GraphError<K> {
    #[error("Node {0} already exists")]
    DuplicateKey(K),

    #[error("Parents not found: {0:?}")]
    MissingParents(BTreeSet<K>),

    #[error("Nodes not found: {0:?}")]
    UnknownNodes(BTreeSet<K>),

    #[error("Nodes were not yet processed: {0:?}")]
    UnprocessedNodes(BTreeSet<K>),
}

/// This cannot be derived without enforcing `Default` bounds on K and V.
impl<K, V> Default for DependencyGraph<K, V> {
    fn default() -> Self {
        Self {
            vertices: Default::default(),
            parents: Default::default(),
            children: Default::default(),
            roots: Default::default(),
            completed: Default::default(),
        }
    }
}

impl<K: Ord + Clone, V: Clone> DependencyGraph<K, V> {
    /// Inserts a new node into the graph.
    ///
    /// # Errors
    ///
    /// Errors if the node already exists, or if any of the parents are not part of the graph.
    ///
    /// This method is atomic.
    pub fn insert(&mut self, key: K, value: V, parents: BTreeSet<K>) -> Result<(), GraphError<K>> {
        if self.vertices.contains_key(&key) {
            return Err(GraphError::DuplicateKey(key));
        }

        let missing_parents = parents
            .iter()
            .filter(|parent| !self.vertices.contains_key(parent))
            .cloned()
            .collect::<BTreeSet<_>>();
        if !missing_parents.is_empty() {
            return Err(GraphError::MissingParents(missing_parents));
        }

        // Inform parents of their new child.
        for parent in &parents {
            self.children.entry(parent.clone()).or_default().insert(key.clone());
        }
        self.vertices.insert(key.clone(), value);
        self.parents.insert(key, parents);

        Ok(())
    }

    /// Requeue the given nodes and their descendents for processing.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the given nodes:
    ///
    ///   - are not part of the graph, or
    ///   - were not previously processed
    ///
    /// This method is atomic.
    pub fn requeue(&mut self, keys: BTreeSet<K>) -> Result<(), GraphError<K>> {
        let missing_nodes = keys
            .iter()
            .filter(|key| !self.vertices.contains_key(key))
            .cloned()
            .collect::<BTreeSet<_>>();
        if !missing_nodes.is_empty() {
            return Err(GraphError::UnknownNodes(missing_nodes));
        }
        let unprocessed = keys.difference(&self.completed).cloned().collect::<BTreeSet<_>>();
        if !unprocessed.is_empty() {
            return Err(GraphError::UnprocessedNodes(unprocessed));
        }

        let mut processed = BTreeSet::new();
        let mut to_process = keys.clone();

        while let Some(key) = to_process.pop_first() {
            self.completed.remove(&key);

            let unprocessed_children = self
                .children
                .get(&key)
                .map(|children| children.difference(&processed))
                .into_iter()
                .flatten()
                .cloned();

            to_process.extend(unprocessed_children);

            processed.insert(key);
        }

        // Only the original keys and the current roots need to be considered as roots.
        //
        // The children of the input keys are disqualified by definition (they're descendents),
        // and current roots must be re-evaluated since their parents may have been requeued.
        std::mem::take(&mut self.roots)
            .into_iter()
            .chain(keys)
            .for_each(|key| self.try_make_root(key));

        Ok(())
    }

    pub fn prune(&mut self, keys: BTreeSet<K>) -> Result<(), GraphError<K>> {
        let missing_nodes = keys
            .iter()
            .filter(|key| !self.vertices.contains_key(key))
            .cloned()
            .collect::<BTreeSet<_>>();
        if !missing_nodes.is_empty() {
            return Err(GraphError::UnknownNodes(missing_nodes));
        }

        let unprocessed = keys.difference(&self.completed).cloned().collect::<BTreeSet<_>>();
        if !unprocessed.is_empty() {
            return Err(GraphError::UnprocessedNodes(unprocessed));
        }

        // TODO: check that no parents are left dangling ie unpruned.

        for key in keys {
            self.vertices.remove(&key);
            self.completed.remove(&key);
            self.parents.remove(&key);

            let children = self.children.remove(&key).unwrap_or_default();

            for child in children {
                if let Some(child) = self.parents.get_mut(&child) {
                    child.remove(&key);
                }
            }
        }

        Ok(())
    }

    pub fn purge(&mut self, keys: BTreeSet<K>) -> Result<BTreeMap<K, V>, GraphError<K>> {
        // TODO: validate keys exist

        let mut visited = keys.clone();
        let mut to_process = keys;
        let mut values = BTreeMap::new();

        while let Some(key) = to_process.pop_first() {
            let value = self.vertices.remove(&key).expect("Must exist");
            values.insert(key.clone(), value);
            self.completed.remove(&key);
            self.roots.remove(&key);

            // Children must also be purged.
            let unvisited_children = self.children.remove(&key).unwrap_or_default();
            let unvisited_children = unvisited_children.difference(&visited).cloned();
            to_process.extend(unvisited_children);

            // Inform parents that this child no longer exists.
            let parents = self.parents.remove(&key).unwrap_or_default();
            for parent in parents {
                if let Some(parent) = self.children.get_mut(&parent) {
                    parent.remove(&key);
                }
            }
        }

        Ok(values)
    }

    // pub fn pop_root(&mut self) -> Option<K> {
    //     let selected = self.roots.pop_first()?;
    //     self.completed.insert(selected.clone());

    //     let children = self.children.get(&selected).cloned().unwrap_or_default();
    //     for child in children {
    //         self.try_make_root(child);
    //     }

    //     Some(selected)
    // }

    // pub fn value(&self, key: &K) -> Option<&V> {
    //     self.vertices.get(key)
    // }

    // pub fn parents(&self, key: &K) -> Option<&BTreeSet<K>> {
    //     self.parents.get(key)
    // }

    /// Adds the node to the `roots` list _IFF_ all of its parents are processed.
    fn try_make_root(&mut self, key: K) {
        let parents_completed = self
            .parents
            .get(&key)
            .into_iter()
            .flatten()
            .all(|parent| self.completed.contains(parent));

        if parents_completed {
            self.roots.insert(key);
        }
    }
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // TEST UTILITIES
    // ================================================================================================

    /// Simplified graph variant where a node's key always equals its value. This is done to make
    /// generating test values simpler.
    type TestGraph = DependencyGraph<u32, u32>;

    impl TestGraph {
        /// Alias for inserting a node with no parents.
        fn insert_root(&mut self, node: u32) -> Result<(), GraphError<u32>> {
            self.insert_with_parents(node, Default::default())
        }

        /// Alias for inserting a node with a single parent.
        fn insert_with_parent(&mut self, node: u32, parent: u32) -> Result<(), GraphError<u32>> {
            self.insert_with_parents(node, [parent].into())
        }

        /// Alias for inserting a node with multiple parents.
        fn insert_with_parents(
            &mut self,
            node: u32,
            parents: BTreeSet<u32>,
        ) -> Result<(), GraphError<u32>> {
            self.insert(node, node, parents)
        }

        /// Marks a root node as completed, adding any valid children to the roots list.
        ///
        /// Panics if the node is not in the roots list.
        fn process(&mut self, node: u32) {
            self.roots.take(&node).expect("Node must be in roots list");
            self.completed.insert(node);

            self.children
                .get(&node)
                .cloned()
                .into_iter()
                .flatten()
                .for_each(|child| self.try_make_root(child));
        }
    }

    // INSERT TESTS
    // ================================================================================================

    #[test]
    fn inserted_nodes_are_considered_for_root() {
        //! Ensure that an inserted node is added to the root list if all parents are already processed.
        let parent_a = 1;
        let parent_b = 2;
        let child_a = 3;
        let child_b = 4;
        let child_c = 5;

        let mut uut = TestGraph::default();
        uut.insert_root(parent_a).unwrap();
        uut.insert_root(parent_b).unwrap();

        // Only process one parent so that some children remain unrootable.
        uut.process(parent_a);

        uut.insert_with_parent(child_a, parent_a).unwrap();
        uut.insert_with_parent(child_b, parent_b).unwrap();
        uut.insert_with_parents(child_c, [parent_a, parent_b].into()).unwrap();

        // Only child_a should be added (in addition to the parents), since the other children
        // are dependent on parent_b which is incomplete.
        let expected_roots = [parent_b, child_a].into();

        assert_eq!(uut.roots, expected_roots);
    }

    #[test]
    fn insert_with_known_parents_succeeds() {
        let parent_a = 10;
        let parent_b = 20;
        let grandfather = 123;
        let uncle = 222;

        let mut uut = TestGraph::default();
        uut.insert_root(grandfather).unwrap();
        uut.insert_root(parent_a).unwrap();
        uut.insert_with_parent(parent_b, grandfather).unwrap();
        uut.insert_with_parent(uncle, grandfather).unwrap();
        uut.insert_with_parents(1, [parent_a, parent_b].into()).unwrap();
    }

    #[test]
    fn insert_duplicate_is_rejected() {
        //! Ensure that inserting a duplicate node
        //!   - results in an error, and
        //!   - does not mutate the state (atomicity)
        const KEY: u32 = 123;
        let mut uut = TestGraph::default();
        uut.insert_root(KEY).unwrap();

        let err = uut.insert_root(KEY).unwrap_err();
        let expected = GraphError::DuplicateKey(KEY);
        assert_eq!(err, expected);

        let mut atomic_reference = TestGraph::default();
        atomic_reference.insert_root(KEY);
        assert_eq!(uut, atomic_reference);
    }

    #[test]
    fn insert_with_all_parents_missing_is_rejected() {
        //! Ensure that inserting a node with unknown parents
        //!   - results in an error, and
        //!   - does not mutate the state (atomicity)
        const MISSING: [u32; 4] = [1, 2, 3, 4];
        let mut uut = TestGraph::default();

        let err = uut.insert_with_parents(0xABC, MISSING.into()).unwrap_err();
        let expected = GraphError::MissingParents(MISSING.into());
        assert_eq!(err, expected);

        let atomic_reference = TestGraph::default();
        assert_eq!(uut, atomic_reference);
    }

    #[test]
    fn insert_with_some_parents_missing_is_rejected() {
        //! Ensure that inserting a node with unknown parents
        //!   - results in an error, and
        //!   - does not mutate the state (atomicity)
        const MISSING: u32 = 123;
        let mut uut = TestGraph::default();

        uut.insert_root(1).unwrap();
        uut.insert_root(2).unwrap();
        uut.insert_root(3).unwrap();

        let atomic_reference = uut.clone();

        let err = uut.insert_with_parents(0xABC, [1, 2, 3, MISSING].into()).unwrap_err();
        let expected = GraphError::MissingParents([MISSING].into());
        assert_eq!(err, expected);
        assert_eq!(uut, atomic_reference);
    }
}
