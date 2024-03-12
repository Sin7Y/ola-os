//! Storage-related logic.

pub(crate) use self::patch::{LoadAncestorsResult, WorkingPatchSet};
pub use self::{
    database::{Database, NodeKeys, Patched, PruneDatabase, PrunePatchSet},
    patch::PatchSet,
    rocksdb::{MerkleTreeColumnFamily, RocksDBWrapper},
};
use crate::{
    hasher::HashTree,
    types::{
        BlockOutput, ChildRef, InternalNode, Key, LeafNode, Manifest, Nibbles, Node, Root,
        TreeEntry, TreeLogEntry, TreeTags, ValueHash,
    },
};

mod database;
mod patch;
mod proofs;
mod rocksdb;
mod serialization;
#[cfg(test)]
mod tests;

/// Tree operation: either inserting a new version or updating an existing one (the latter is only
/// used during tree recovery).
#[derive(Debug, Clone, Copy)]
enum Operation {
    Insert,
    Update,
}

/// Mutable storage encapsulating AR16MT update logic.
#[derive(Debug)]
struct TreeUpdater {
    patch_set: WorkingPatchSet,
    pub new_leaves: u64,
}

impl TreeUpdater {
    fn new(version: u64, root: Root) -> Self {
        Self {
            patch_set: WorkingPatchSet::new(version, root),
            new_leaves: 0,
        }
    }

    fn set_root_node(&mut self, node: Node) {
        self.patch_set.insert(Nibbles::EMPTY, node);
    }

    /// Gets a node to be mutated.
    fn get_mut(&mut self, nibbles: &Nibbles) -> Option<&mut Node> {
        self.patch_set.get_mut(nibbles)
    }

    fn insert_node(&mut self, nibbles: Nibbles, node: impl Into<Node>, is_new: bool) {
        let node = node.into();
        if let Node::Internal(_) = node {
            debug_assert!(is_new); // internal nodes are never moved
        }
        self.patch_set.insert(nibbles, node);
    }

    fn load_ancestors<DB: Database + ?Sized>(
        &mut self,
        sorted_keys: &SortedKeys,
        db: &DB,
    ) -> Vec<Nibbles> {
        let LoadAncestorsResult {
            longest_prefixes, ..
        } = self.patch_set.load_ancestors(sorted_keys, db);

        longest_prefixes
    }

    /// Loads the greatest key from the database.
    fn load_greatest_key<DB: Database + ?Sized>(&mut self, db: &DB) -> Option<(LeafNode, Nibbles)> {
        let (leaf, load_result) = self.patch_set.load_greatest_key(db)?;
        assert_eq!(load_result.longest_prefixes.len(), 1);
        Some((leaf, load_result.longest_prefixes[0]))
    }

    fn insert(
        &mut self,
        entry: TreeEntry,
        parent_nibbles: &Nibbles,
    ) -> (TreeLogEntry, NewLeafData) {
        let version = self.patch_set.root_version();
        let key = entry.key;

        let traverse_outcome = self.patch_set.traverse(key, parent_nibbles);
        let (log, leaf_data) = match traverse_outcome {
            TraverseOutcome::LeafMatch(nibbles, mut leaf) => {
                let log = TreeLogEntry::update(leaf.leaf_index, leaf.value_hash);
                leaf.update_from(entry);
                self.patch_set.insert(nibbles, leaf.into());
                (log, NewLeafData::new(nibbles, leaf))
            }

            TraverseOutcome::LeafMismatch(nibbles, leaf) => {
                self.update_moved_leaf_ref(&nibbles);

                let mut nibble_idx = nibbles.nibble_count();
                loop {
                    let moved_leaf_nibble = Nibbles::nibble(&leaf.full_key, nibble_idx);
                    let new_leaf_nibble = Nibbles::nibble(&key, nibble_idx);
                    let mut node = InternalNode::default();
                    if moved_leaf_nibble == new_leaf_nibble {
                        // Insert a path of internal nodes with a single child.
                        node.insert_child_ref(new_leaf_nibble, ChildRef::internal(version));
                    } else {
                        // Insert a diverging internal node with 2 children for the existing
                        // and the new leaf.
                        node.insert_child_ref(new_leaf_nibble, ChildRef::leaf(version));
                        node.insert_child_ref(moved_leaf_nibble, ChildRef::leaf(version));
                    }
                    let node_nibbles = Nibbles::new(&key, nibble_idx);
                    self.insert_node(node_nibbles, node, true);
                    if moved_leaf_nibble != new_leaf_nibble {
                        break;
                    }
                    nibble_idx += 1;
                }

                let new_leaf = LeafNode::new(entry);
                let new_leaf_nibbles = Nibbles::new(&key, nibble_idx + 1);
                let leaf_data = NewLeafData::new(new_leaf_nibbles, new_leaf);
                let moved_leaf_nibbles = Nibbles::new(&leaf.full_key, nibble_idx + 1);
                let leaf_data = leaf_data.with_adjacent_leaf(moved_leaf_nibbles, leaf);
                (TreeLogEntry::Inserted, leaf_data)
            }

            TraverseOutcome::MissingChild(nibbles) if nibbles.nibble_count() == 0 => {
                // The root is currently empty; we replace it with a leaf.
                let root_leaf = LeafNode::new(entry);
                self.set_root_node(root_leaf.into());
                let leaf_data = NewLeafData::new(Nibbles::EMPTY, root_leaf);
                (TreeLogEntry::Inserted, leaf_data)
            }

            TraverseOutcome::MissingChild(nibbles) => {
                let (parent_nibbles, last_nibble) = nibbles.split_last().unwrap();
                let Some(Node::Internal(parent)) = self.get_mut(&parent_nibbles) else {
                    unreachable!("Node parent must be an internal node");
                };
                parent.insert_child_ref(last_nibble, ChildRef::leaf(version));
                let new_leaf = LeafNode::new(entry);
                let leaf_data = NewLeafData::new(nibbles, new_leaf);
                (TreeLogEntry::Inserted, leaf_data)
            }
        };

        if matches!(log, TreeLogEntry::Inserted { .. }) {
            self.insert_node(leaf_data.nibbles, leaf_data.leaf, true);
        }
        if let Some((nibbles, leaf)) = leaf_data.adjacent_leaf {
            self.insert_node(nibbles, leaf, false);
        }

        // Traverse nodes up to the root level and update `ChildRef.version`.
        let mut cursor = traverse_outcome.position();
        while let Some((parent_nibbles, last_nibble)) = cursor.split_last() {
            let child_ref = self
                .patch_set
                .child_ref_mut(&parent_nibbles, last_nibble)
                .unwrap();
            child_ref.version = child_ref.version.max(version);
            cursor = parent_nibbles;
        }

        (log, leaf_data)
    }

    fn update_moved_leaf_ref(&mut self, leaf_nibbles: &Nibbles) {
        if let Some((parent_nibbles, last_nibble)) = leaf_nibbles.split_last() {
            let child_ref = self
                .patch_set
                .child_ref_mut(&parent_nibbles, last_nibble)
                .unwrap();
            child_ref.is_leaf = false;
        }
    }
}

/// [`TreeUpdater`] together with a link to the database.
#[derive(Debug)]
pub(crate) struct Storage<'a, DB: ?Sized> {
    db: &'a DB,
    hasher: &'a dyn HashTree,
    manifest: Manifest,
    leaf_count: u64,
    operation: Operation,
    updater: TreeUpdater,
}

impl<'a, DB: Database + ?Sized> Storage<'a, DB> {
    /// Creates storage for a new version of the tree.
    pub fn new(
        db: &'a DB,
        hasher: &'a dyn HashTree,
        version: u64,
        create_new_version: bool,
    ) -> Self {
        let mut manifest = db.manifest().unwrap_or_default();
        if manifest.tags.is_none() {
            manifest.tags = Some(TreeTags::new(hasher));
        }
        manifest.version_count = version + 1;

        let base_version = if create_new_version {
            version.checked_sub(1)
        } else {
            Some(version)
        };
        let root = if let Some(base_version) = base_version {
            db.root(base_version).unwrap_or(Root::Empty)
        } else {
            Root::Empty
        };

        Self {
            db,
            hasher,
            manifest,
            leaf_count: root.leaf_count(),
            operation: if create_new_version {
                Operation::Insert
            } else {
                Operation::Update
            },
            updater: TreeUpdater::new(version, root),
        }
    }

    /// Extends the Merkle tree in the lightweight operation mode, without intermediate hash
    /// computations.
    pub fn extend(mut self, entries: Vec<TreeEntry>) -> (BlockOutput, PatchSet) {
        let sorted_keys = SortedKeys::new(entries.iter().map(|entry| entry.key));
        let parent_nibbles = self.updater.load_ancestors(&sorted_keys, self.db);

        let mut logs = Vec::with_capacity(entries.len());
        for (entry, parent_nibbles) in entries.into_iter().zip(parent_nibbles) {
            let (log, _) = self.updater.insert(entry, &parent_nibbles);
            if matches!(log, TreeLogEntry::Inserted) {
                self.leaf_count += 1;
            }
            logs.push(log);
        }

        let leaf_count = self.leaf_count;
        let (root_hash, patch) = self.finalize();
        let output = BlockOutput {
            root_hash,
            leaf_count,
            logs,
        };
        (output, patch)
    }

    pub fn greatest_key(mut self) -> Option<Key> {
        Some(self.updater.load_greatest_key(self.db)?.0.full_key)
    }

    pub fn extend_during_linear_recovery(mut self, recovery_entries: Vec<TreeEntry>) -> PatchSet {
        let (mut prev_key, mut prev_nibbles) = match self.updater.load_greatest_key(self.db) {
            Some((leaf, nibbles)) => (Some(leaf.full_key), nibbles),
            None => (None, Nibbles::EMPTY),
        };

        for entry in recovery_entries {
            if let Some(prev_key) = prev_key {
                assert!(
                    entry.key > prev_key,
                    "Recovery entries must be ordered by increasing key (previous key: {prev_key:0>64x}, \
                     offending entry: {entry:?})"
                );
            }
            prev_key = Some(entry.key);

            let key_nibbles = Nibbles::new(&entry.key, prev_nibbles.nibble_count());
            let parent_nibbles = prev_nibbles.common_prefix(&key_nibbles);
            let (_, new_leaf) = self.updater.insert(entry, &parent_nibbles);
            prev_nibbles = new_leaf.nibbles;
            self.leaf_count += 1;
        }

        let (_, patch) = self.finalize();
        patch
    }

    pub fn extend_during_random_recovery(mut self, recovery_entries: Vec<TreeEntry>) -> PatchSet {
        let sorted_keys = SortedKeys::new(recovery_entries.iter().map(|entry| entry.key));
        let parent_nibbles = self.updater.load_ancestors(&sorted_keys, self.db);

        for (entry, parent_nibbles) in recovery_entries.into_iter().zip(parent_nibbles) {
            self.updater.insert(entry, &parent_nibbles);
        }

        let (_, patch) = self.finalize();
        patch
    }

    fn finalize(self) -> (ValueHash, PatchSet) {
        self.updater
            .patch_set
            .finalize(self.manifest, self.leaf_count, self.operation, self.hasher)
    }
}

/// Sorted [`Key`]s together with their indices in the block.
#[derive(Debug)]
pub(crate) struct SortedKeys(Vec<(usize, Key)>);

impl SortedKeys {
    pub fn new(keys: impl Iterator<Item = Key>) -> Self {
        let mut keys: Vec<_> = keys.enumerate().collect();
        keys.sort_unstable_by_key(|(_, key)| *key);
        Self(keys)
    }
}

/// Outcome of traversing a tree for a specific key.
#[derive(Debug)]
enum TraverseOutcome {
    /// The matching leaf is present in the tree.
    LeafMatch(Nibbles, LeafNode),
    /// There traversal ends in a leaf with mismatched full key.
    LeafMismatch(Nibbles, LeafNode),
    /// The traversal cannot proceed because of a missing child ref in an internal node.
    MissingChild(Nibbles),
}

impl TraverseOutcome {
    /// Returns the final position during the traversal.
    fn position(&self) -> Nibbles {
        match self {
            Self::LeafMatch(nibbles, _)
            | Self::LeafMismatch(nibbles, _)
            | Self::MissingChild(nibbles) => *nibbles,
        }
    }
}

/// Information about the newly inserted / updated leaf. Can also include information about
/// an adjacent leaf moved down the tree.
#[derive(Debug)]
struct NewLeafData {
    /// Nibbles for the new leaf node.
    nibbles: Nibbles,
    /// The new leaf node.
    leaf: LeafNode,
    /// Nibbles and node for the adjacent leaf moved down the tree.
    adjacent_leaf: Option<(Nibbles, LeafNode)>,
}

impl NewLeafData {
    fn new(nibbles: Nibbles, leaf: LeafNode) -> Self {
        Self {
            nibbles,
            leaf,
            adjacent_leaf: None,
        }
    }

    fn with_adjacent_leaf(mut self, nibbles: Nibbles, leaf: LeafNode) -> Self {
        self.adjacent_leaf = Some((nibbles, leaf));
        self
    }
}
