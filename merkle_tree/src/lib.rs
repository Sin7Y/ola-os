// Linter settings.
#![warn(missing_debug_implementations, missing_docs, bare_trait_objects)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(
    clippy::must_use_candidate,
    clippy::module_name_repetitions,
    clippy::doc_markdown // frequent false positive: RocksDB
)]

use ola_utils::hash::PoseidonHasher;

pub use crate::{
    errors::NoVersionError,
    hasher::{HashTree, TreeRangeDigest},
    pruning::{MerkleTreePruner, MerkleTreePrunerHandle},
    storage::{
        Database, MerkleTreeColumnFamily, PatchSet, Patched, PruneDatabase, PrunePatchSet,
        RocksDBWrapper,
    },
    types::{
        BlockOutput, BlockOutputWithProofs, Key, TreeEntry, TreeEntryWithProof, TreeInstruction,
        TreeLogEntry, TreeLogEntryWithProof, ValueHash,
    },
};
use crate::{hasher::HasherWithStats, storage::Storage, types::Root};

mod consistency;
pub mod domain;
mod errors;
mod getters;
mod hasher;
mod pruning;
pub mod recovery;
mod storage;
mod types;
mod utils;

#[doc(hidden)]
pub mod unstable {
    pub use crate::{
        errors::DeserializeError,
        types::{Manifest, Node, NodeKey, Root},
    };
}

#[derive(Debug)]
pub struct MerkleTree<DB, H = PoseidonHasher> {
    db: DB,
    hasher: H,
}

impl<DB: Database> MerkleTree<DB> {
    pub fn new(db: DB) -> Self {
        Self::with_hasher(db, PoseidonHasher)
    }
}

impl<DB: Database, H: HashTree> MerkleTree<DB, H> {
    pub fn with_hasher(db: DB, hasher: H) -> Self {
        let tags = db.manifest().and_then(|manifest| manifest.tags);
        if let Some(tags) = tags {
            tags.assert_consistency(&hasher, false);
        }
        Self { db, hasher }
    }

    pub fn root_hash(&self, version: u64) -> Option<ValueHash> {
        let root = self.root(version)?;
        let Root::Filled { node, .. } = root else {
            return Some(self.hasher.empty_tree_hash());
        };
        Some(node.hash(&mut HasherWithStats::new(&self.hasher), 0))
    }

    pub(crate) fn root(&self, version: u64) -> Option<Root> {
        self.db.root(version)
    }

    /// Returns the latest version of the tree present in the database, or `None` if
    /// no versions are present yet.
    pub fn latest_version(&self) -> Option<u64> {
        self.db.manifest()?.version_count.checked_sub(1)
    }

    /// Returns the root hash for the latest version of the tree.
    pub fn latest_root_hash(&self) -> ValueHash {
        let root_hash = self
            .latest_version()
            .and_then(|version| self.root_hash(version));
        root_hash.unwrap_or_else(|| self.hasher.empty_tree_hash())
    }

    /// Returns the latest-versioned root node.
    pub(crate) fn latest_root(&self) -> Root {
        let root = self.latest_version().and_then(|version| self.root(version));
        root.unwrap_or(Root::Empty)
    }

    /// Removes the most recent versions from the database.
    ///
    /// The current implementation does not actually remove node data for the removed versions
    /// since it's likely to be reused in the future (especially upper-level internal nodes).
    pub fn truncate_recent_versions(&mut self, retained_version_count: u64) {
        let mut manifest = self.db.manifest().unwrap_or_default();
        if manifest.version_count > retained_version_count {
            manifest.version_count = retained_version_count;
            let patch = PatchSet::from_manifest(manifest);
            self.db.apply_patch(patch);
        }
    }

    /// Extends this tree by creating its new version.
    ///
    /// # Return value
    ///
    /// Returns information about the update such as the final tree hash.
    pub fn extend(&mut self, entries: Vec<TreeEntry>) -> BlockOutput {
        let next_version = self.db.manifest().unwrap_or_default().version_count;
        let storage = Storage::new(&self.db, &self.hasher, next_version, true);
        let (output, patch) = storage.extend(entries);
        self.db.apply_patch(patch);
        output
    }

    /// Extends this tree by creating its new version, computing an authenticity Merkle proof
    /// for each provided instruction.
    ///
    /// # Return value
    ///
    /// Returns information about the update such as the final tree hash and proofs for each input
    /// instruction.
    pub fn extend_with_proofs(
        &mut self,
        instructions: Vec<TreeInstruction>,
    ) -> BlockOutputWithProofs {
        let next_version = self.db.manifest().unwrap_or_default().version_count;
        let storage = Storage::new(&self.db, &self.hasher, next_version, true);
        let (output, patch) = storage.extend_with_proofs(instructions);
        self.db.apply_patch(patch);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TreeTags;

    #[test]
    #[should_panic(expected = "Unsupported tree architecture `AR64MT`, expected `AR16MT`")]
    fn tree_architecture_mismatch() {
        let mut db = PatchSet::default();
        db.manifest_mut().tags = Some(TreeTags {
            architecture: "AR64MT".to_owned(),
            depth: 256,
            hasher: "blake2s256".to_string(),
            is_recovering: false,
        });

        MerkleTree::new(db);
    }

    #[test]
    #[should_panic(expected = "Unexpected tree depth: expected 256, got 128")]
    fn tree_depth_mismatch() {
        let mut db = PatchSet::default();
        db.manifest_mut().tags = Some(TreeTags {
            architecture: "AR16MT".to_owned(),
            depth: 128,
            hasher: "blake2s256".to_string(),
            is_recovering: false,
        });

        MerkleTree::new(db);
    }

    #[test]
    #[should_panic(expected = "Mismatch between the provided tree hasher `blake2s256`")]
    fn hasher_mismatch() {
        let mut db = PatchSet::default();
        db.manifest_mut().tags = Some(TreeTags {
            architecture: "AR16MT".to_owned(),
            depth: 256,
            hasher: "sha256".to_string(),
            is_recovering: false,
        });

        MerkleTree::new(db);
    }
}
