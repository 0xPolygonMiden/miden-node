use super::*;

// TEST UTILITIES
// ================================================================================================

/// Simplified graph variant where a node's key always equals its value. This is done to make
/// generating test values simpler.
type TestGraph = DependencyGraph<u32, u32>;

impl TestGraph {
    /// Alias for inserting a node with no parents.
    fn insert_with_no_parents(&mut self, node: u32) -> Result<(), GraphError<u32>> {
        self.insert_with_parents(node, BTreeSet::default())
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
        self.insert_pending(node, parents)
    }

    /// Alias for promoting nodes with the same value as the key.
    fn promote(&mut self, nodes: impl IntoIterator<Item = u32>) -> Result<(), GraphError<u32>> {
        for node in nodes {
            self.promote_pending(node, node)?;
        }
        Ok(())
    }

    /// Promotes all nodes in the pending list with value=key.
    fn promote_all(&mut self) {
        // SAFETY: these are definitely pending nodes.
        self.promote(self.pending.clone()).unwrap();
    }

    /// Calls `process_root` until all nodes have been processed.
    fn process_all(&mut self) {
        while let Some(root) = self.roots().first().copied() {
            // SAFETY: this is definitely a root since we just took it from there :)
            self.process_root(root).unwrap();
        }
    }
}

// PROMOTE TESTS
// ================================================================================================

#[test]
fn promoted_nodes_are_considered_for_root() {
    //! Ensure that a promoted node is added to the root list if all parents are already
    //! processed.
    let parent_a = 1;
    let parent_b = 2;
    let child_a = 3;
    let child_b = 4;
    let child_c = 5;

    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(parent_a).unwrap();
    uut.insert_with_no_parents(parent_b).unwrap();
    uut.promote_all();

    // Only process one parent so that some children remain unrootable.
    uut.process_root(parent_a).unwrap();

    uut.insert_with_parent(child_a, parent_a).unwrap();
    uut.insert_with_parent(child_b, parent_b).unwrap();
    uut.insert_with_parents(child_c, [parent_a, parent_b].into()).unwrap();

    uut.promote_all();

    // Only child_a should be added (in addition to the parents), since the other children
    // are dependent on parent_b which is incomplete.
    let expected_roots = [parent_b, child_a].into();

    assert_eq!(uut.roots, expected_roots);
}

#[test]
fn pending_nodes_are_not_considered_for_root() {
    //! Ensure that an unpromoted node is _not_ added to the root list even if all parents are
    //! already processed.
    let parent_a = 1;
    let parent_b = 2;
    let child_a = 3;
    let child_b = 4;
    let child_c = 5;

    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(parent_a).unwrap();
    uut.insert_with_no_parents(parent_b).unwrap();
    uut.promote_all();
    uut.process_all();

    uut.insert_with_parent(child_a, parent_a).unwrap();
    uut.insert_with_parent(child_b, parent_b).unwrap();
    uut.insert_with_parents(child_c, [parent_a, parent_b].into()).unwrap();

    uut.promote([child_b]).unwrap();

    // Only child b is valid as it was promoted.
    let expected = [child_b].into();

    assert_eq!(uut.roots, expected);
}

#[test]
fn promoted_nodes_are_moved() {
    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(123).unwrap();

    assert!(uut.pending.contains(&123));
    assert!(!uut.vertices.contains_key(&123));

    uut.promote_pending(123, 123).unwrap();

    assert!(!uut.pending.contains(&123));
    assert!(uut.vertices.contains_key(&123));
}

#[test]
fn promote_rejects_already_promoted_nodes() {
    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(123).unwrap();
    uut.promote_all();

    let err = uut.promote_pending(123, 123).unwrap_err();
    let expected = GraphError::InvalidPendingNode(123);
    assert_eq!(err, expected);
}

#[test]
fn promote_rejects_unknown_nodes() {
    let err = TestGraph::default().promote_pending(123, 123).unwrap_err();
    let expected = GraphError::InvalidPendingNode(123);
    assert_eq!(err, expected);
}

// INSERT TESTS
// ================================================================================================

#[test]
fn insert_with_known_parents_succeeds() {
    let parent_a = 10;
    let parent_b = 20;
    let grandfather = 123;
    let uncle = 222;

    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(grandfather).unwrap();
    uut.insert_with_no_parents(parent_a).unwrap();
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
    uut.insert_with_no_parents(KEY).unwrap();

    let err = uut.insert_with_no_parents(KEY).unwrap_err();
    let expected = GraphError::DuplicateKey(KEY);
    assert_eq!(err, expected);

    let mut atomic_reference = TestGraph::default();
    atomic_reference.insert_with_no_parents(KEY).unwrap();
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

    uut.insert_with_no_parents(1).unwrap();
    uut.insert_with_no_parents(2).unwrap();
    uut.insert_with_no_parents(3).unwrap();

    let atomic_reference = uut.clone();

    let err = uut.insert_with_parents(0xABC, [1, 2, 3, MISSING].into()).unwrap_err();
    let expected = GraphError::MissingParents([MISSING].into());
    assert_eq!(err, expected);
    assert_eq!(uut, atomic_reference);
}

// REVERT TESTS
// ================================================================================================

#[test]
fn reverting_unprocessed_nodes_is_rejected() {
    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(1).unwrap();
    uut.insert_with_no_parents(2).unwrap();
    uut.insert_with_no_parents(3).unwrap();
    uut.promote_all();
    uut.process_root(1).unwrap();

    let err = uut.revert_subgraphs([1, 2, 3].into()).unwrap_err();
    let expected = GraphError::UnprocessedNodes([2, 3].into());

    assert_eq!(err, expected);
}

#[test]
fn reverting_unknown_nodes_is_rejected() {
    let err = TestGraph::default().revert_subgraphs([1].into()).unwrap_err();
    let expected = GraphError::UnknownNodes([1].into());
    assert_eq!(err, expected);
}

#[test]
fn reverting_resets_the_entire_subgraph() {
    //! Reverting should reset the state to before any of the nodes where processed.
    let grandparent = 1;
    let parent_a = 2;
    let parent_b = 3;
    let child_a = 4;
    let child_b = 5;
    let child_c = 6;

    let disjoint = 7;

    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(grandparent).unwrap();
    uut.insert_with_no_parents(disjoint).unwrap();
    uut.insert_with_parent(parent_a, grandparent).unwrap();
    uut.insert_with_parent(parent_b, grandparent).unwrap();
    uut.insert_with_parent(child_a, parent_a).unwrap();
    uut.insert_with_parent(child_b, parent_b).unwrap();
    uut.insert_with_parents(child_c, [parent_a, parent_b].into()).unwrap();

    uut.promote([disjoint, grandparent, parent_a, parent_b, child_a, child_c])
        .unwrap();
    uut.process_root(disjoint).unwrap();

    let reference = uut.clone();

    uut.process_all();
    uut.revert_subgraphs([grandparent].into()).unwrap();

    assert_eq!(uut, reference);
}

#[test]
fn reverting_reevaluates_roots() {
    //! Node reverting from processed to unprocessed should cause the root nodes to be
    //! re-evaluated. Only nodes with all parents processed should remain in the set.
    let disjoint_parent = 1;
    let disjoint_child = 2;

    let parent_a = 3;
    let parent_b = 4;
    let child_a = 5;
    let child_b = 6;

    let partially_disjoin_child = 7;

    let mut uut = TestGraph::default();
    // This pair of nodes should not be impacted by the reverted subgraph.
    uut.insert_with_no_parents(disjoint_parent).unwrap();
    uut.insert_with_parent(disjoint_child, disjoint_parent).unwrap();

    uut.insert_with_no_parents(parent_a).unwrap();
    uut.insert_with_no_parents(parent_b).unwrap();
    uut.insert_with_parent(child_a, parent_a).unwrap();
    uut.insert_with_parent(child_b, parent_b).unwrap();
    uut.insert_with_parents(partially_disjoin_child, [disjoint_parent, parent_a].into())
        .unwrap();

    // Since we are reverting the other parents, we expect the roots to match the current state.
    uut.promote_all();
    uut.process_root(disjoint_parent).unwrap();
    let reference = uut.roots().clone();

    uut.process_root(parent_a).unwrap();
    uut.process_root(parent_b).unwrap();
    uut.revert_subgraphs([parent_a, parent_b].into()).unwrap();

    assert_eq!(uut.roots(), &reference);
}

// PRUNING TESTS
// ================================================================================================

#[test]
fn pruned_nodes_are_nonextant() {
    //! Checks that processed and then pruned nodes behave as if they never existed in the
    //! graph. We test this by comparing it to a reference graph created without these ancestor
    //! nodes.
    let ancestor_a = 1;
    let ancestor_b = 2;

    let child_a = 3;
    let child_b = 4;
    let child_both = 5;

    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(ancestor_a).unwrap();
    uut.insert_with_no_parents(ancestor_b).unwrap();
    uut.insert_with_parent(child_a, ancestor_a).unwrap();
    uut.insert_with_parent(child_b, ancestor_b).unwrap();
    uut.insert_with_parents(child_both, [ancestor_a, ancestor_b].into()).unwrap();
    uut.promote_all();

    uut.process_root(ancestor_a).unwrap();
    uut.process_root(ancestor_b).unwrap();
    uut.prune_processed([ancestor_a, ancestor_b].into()).unwrap();

    let mut reference = TestGraph::default();
    reference.insert_with_no_parents(child_a).unwrap();
    reference.insert_with_no_parents(child_b).unwrap();
    reference.insert_with_no_parents(child_both).unwrap();
    reference.promote_all();

    assert_eq!(uut, reference);
}

#[test]
fn pruning_unknown_nodes_is_rejected() {
    let err = TestGraph::default().prune_processed([1].into()).unwrap_err();
    let expected = GraphError::UnknownNodes([1].into());
    assert_eq!(err, expected);
}

#[test]
fn pruning_unprocessed_nodes_is_rejected() {
    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(1).unwrap();
    uut.promote_all();

    let err = uut.prune_processed([1].into()).unwrap_err();
    let expected = GraphError::UnprocessedNodes([1].into());
    assert_eq!(err, expected);
}

#[test]
fn pruning_cannot_leave_parents_dangling() {
    //! Pruning processed nodes must always prune all parent nodes as well. No parent node may
    //! be left behind.
    let dangling = 1;
    let pruned = 2;
    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(dangling).unwrap();
    uut.insert_with_parent(pruned, dangling).unwrap();
    uut.promote_all();
    uut.process_all();

    let err = uut.prune_processed([pruned].into()).unwrap_err();
    let expected = GraphError::DanglingNodes([dangling].into());
    assert_eq!(err, expected);
}

// PURGING TESTS
// ================================================================================================

#[test]
fn purging_subgraph_handles_internal_nodes() {
    //! Purging a subgraph should correctly handle nodes already deleted within that subgraph.
    //!
    //! This is a concern for errors as we are deleting parts of the subgraph while we are
    //! iterating through the nodes to purge. This means its likely a node will already have
    //! been deleted before processing it as an input.
    //!
    //! We can force this to occur by re-ordering the inputs relative to the actual dependency
    //! order. This means this test is a bit weaker because it relies on implementation details.

    let ancestor_a = 1;
    let ancestor_b = 2;
    let parent_a = 3;
    let parent_b = 4;
    let child_a = 5;
    // This should be purged prior to parent_a. Relies on the fact that we are iterating over a
    // btree which is ordered by value.
    let child_b = 0;
    let child_c = 6;

    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(ancestor_a).unwrap();
    uut.insert_with_no_parents(ancestor_b).unwrap();
    uut.insert_with_parent(parent_a, ancestor_a).unwrap();
    uut.insert_with_parent(parent_b, ancestor_b).unwrap();
    uut.insert_with_parents(child_a, [ancestor_a, parent_a].into()).unwrap();
    uut.insert_with_parents(child_b, [parent_a, parent_b].into()).unwrap();
    uut.insert_with_parent(child_c, parent_b).unwrap();

    uut.purge_subgraphs([child_b, parent_a].into()).unwrap();

    let mut reference = TestGraph::default();
    reference.insert_with_no_parents(ancestor_a).unwrap();
    reference.insert_with_no_parents(ancestor_b).unwrap();
    reference.insert_with_parent(parent_b, ancestor_b).unwrap();
    reference.insert_with_parent(child_c, parent_b).unwrap();

    assert_eq!(uut, reference);
}

#[test]
fn purging_removes_all_descendents() {
    let ancestor_a = 1;
    let ancestor_b = 2;
    let parent_a = 3;
    let parent_b = 4;
    let child_a = 5;
    let child_b = 6;
    let child_c = 7;

    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(ancestor_a).unwrap();
    uut.insert_with_no_parents(ancestor_b).unwrap();
    uut.insert_with_parent(parent_a, ancestor_a).unwrap();
    uut.insert_with_parent(parent_b, ancestor_b).unwrap();
    uut.insert_with_parents(child_a, [ancestor_a, parent_a].into()).unwrap();
    uut.insert_with_parents(child_b, [parent_a, parent_b].into()).unwrap();
    uut.insert_with_parent(child_c, parent_b).unwrap();

    uut.purge_subgraphs([parent_a].into()).unwrap();

    let mut reference = TestGraph::default();
    reference.insert_with_no_parents(ancestor_a).unwrap();
    reference.insert_with_no_parents(ancestor_b).unwrap();
    reference.insert_with_parent(parent_b, ancestor_b).unwrap();
    reference.insert_with_parent(child_c, parent_b).unwrap();

    assert_eq!(uut, reference);
}

// PROCESSING TESTS
// ================================================================================================

#[test]
fn process_root_evaluates_children_as_roots() {
    let parent_a = 1;
    let parent_b = 2;
    let child_a = 3;
    let child_b = 4;
    let child_c = 5;

    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(parent_a).unwrap();
    uut.insert_with_no_parents(parent_b).unwrap();
    uut.insert_with_parent(child_a, parent_a).unwrap();
    uut.insert_with_parent(child_b, parent_b).unwrap();
    uut.insert_with_parents(child_c, [parent_a, parent_b].into()).unwrap();
    uut.promote_all();

    uut.process_root(parent_a).unwrap();
    assert_eq!(uut.roots(), &[parent_b, child_a].into());
}

#[test]
fn process_root_rejects_non_root_node() {
    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(1).unwrap();
    uut.insert_with_parent(2, 1).unwrap();
    uut.promote_all();

    let err = uut.process_root(2).unwrap_err();
    let expected = GraphError::InvalidRootNode(2);
    assert_eq!(err, expected);
}

#[test]
fn process_root_cannot_reprocess_same_node() {
    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(1).unwrap();
    uut.promote_all();
    uut.process_root(1).unwrap();

    let err = uut.process_root(1).unwrap_err();
    let expected = GraphError::InvalidRootNode(1);
    assert_eq!(err, expected);
}

#[test]
fn processing_a_queue_graph() {
    //! Creates a queue graph and ensures that nodes processed in FIFO order.
    let nodes = (0..10).collect::<Vec<_>>();

    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(nodes[0]).unwrap();
    for pairs in nodes.windows(2) {
        let (parent, id) = (pairs[0], pairs[1]);
        uut.insert_with_parent(id, parent).unwrap();
    }
    uut.promote_all();

    let mut ordered_roots = Vec::<u32>::new();
    for _ in &nodes {
        let current_roots = uut.roots().clone();
        ordered_roots.extend(&current_roots);

        for root in current_roots {
            uut.process_root(root).unwrap();
        }
    }

    assert_eq!(ordered_roots, nodes);
}

#[test]
fn processing_and_root_tracking() {
    //! Creates a somewhat arbitrarily connected graph and ensures that roots are tracked as
    //! expected as the they are processed.
    let ancestor_a = 1;
    let ancestor_b = 2;
    let parent_a = 3;
    let parent_b = 4;
    let child_a = 5;
    let child_b = 6;
    let child_c = 7;

    let mut uut = TestGraph::default();
    uut.insert_with_no_parents(ancestor_a).unwrap();
    uut.insert_with_no_parents(ancestor_b).unwrap();
    uut.insert_with_parent(parent_a, ancestor_a).unwrap();
    uut.insert_with_parent(parent_b, ancestor_b).unwrap();
    uut.insert_with_parents(child_a, [ancestor_a, parent_a].into()).unwrap();
    uut.insert_with_parents(child_b, [parent_a, parent_b].into()).unwrap();
    uut.insert_with_parent(child_c, parent_b).unwrap();
    uut.promote_all();

    assert_eq!(uut.roots(), &[ancestor_a, ancestor_b].into());

    uut.process_root(ancestor_a).unwrap();
    assert_eq!(uut.roots(), &[ancestor_b, parent_a].into());

    uut.process_root(ancestor_b).unwrap();
    assert_eq!(uut.roots(), &[parent_a, parent_b].into());

    uut.process_root(parent_a).unwrap();
    assert_eq!(uut.roots(), &[parent_b, child_a].into());

    uut.process_root(parent_b).unwrap();
    assert_eq!(uut.roots(), &[child_a, child_b, child_c].into());

    uut.process_root(child_a).unwrap();
    assert_eq!(uut.roots(), &[child_b, child_c].into());

    uut.process_root(child_b).unwrap();
    assert_eq!(uut.roots(), &[child_c].into());

    uut.process_root(child_c).unwrap();
    assert!(uut.roots().is_empty());
}
