use crate::TreeError;
use itertools::Itertools;
use log::debug;
use ola_types::{
    merkle_tree::{
        tree_key_to_u256, tree_key_to_u8_arr, tree_value_to_h256, u8_arr_to_tree_key, LeafIndices,
        LevelIndex, TreeOperation,
    },
    storage::writes::{InitialStorageWrite, RepeatedStorageWrite},
};
use ola_utils::convert::{
    deserialize_block_number, deserialize_leaf_index, serialize_block_number, serialize_leaf_index,
    serialize_tree_leaf,
};
use olaos_storage::{db::NamedColumnFamily, RocksDB};
use olavm_core::types::merkle_tree::{TreeKey, ZkHash};
use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
};

const BLOCK_NUMBER_KEY: &[u8; 12] = b"block_number";
const LEAF_INDEX_KEY: &[u8; 10] = b"leaf_index";

/// RocksDB column families used by the tree.
#[derive(Debug, Clone, Copy)]
pub enum MerkleTreeColumnFamily {
    Tree,
    LeafIndices,
}

impl NamedColumnFamily for MerkleTreeColumnFamily {
    const DB_NAME: &'static str = "merkle_tree";
    const ALL: &'static [Self] = &[Self::Tree, Self::LeafIndices];

    fn name(&self) -> &'static str {
        match self {
            Self::Tree => "default",
            Self::LeafIndices => "leaf_indices",
        }
    }

    fn requires_tuning(&self) -> bool {
        matches!(self, Self::Tree)
    }
}

/// Storage wrapper around RocksDB.
/// Stores hashes of branch nodes in merkle tree and current block number
#[derive(Debug)]
pub struct Storage {
    db: RocksDB<MerkleTreeColumnFamily>,
    pub(crate) put_pending_patch: HashMap<Vec<u8>, Vec<u8>>,
    pub(crate) delete_pending_patch: Vec<Vec<u8>>,
}

impl Storage {
    pub fn new(db: RocksDB<MerkleTreeColumnFamily>) -> Self {
        Self {
            db,
            put_pending_patch: HashMap::new(),
            delete_pending_patch: vec![],
        }
    }

    /// Fetches hashes of merkle tree branches from db
    pub fn hashes<'a, I: 'a>(&'a self, keys: I) -> Vec<Option<Vec<u8>>>
    where
        I: IntoIterator<Item = &'a LevelIndex>,
    {
        self.db
            .multi_get_cf(
                MerkleTreeColumnFamily::LeafIndices,
                keys.into_iter().map(LevelIndex::bin_key),
            )
            .into_iter()
            .map(|result| match result {
                Ok(Some(slice)) => Some(slice.as_ref().to_vec()),
                _ => None,
            })
            .collect()
    }

    pub fn hash(&self, key: &LevelIndex) -> Option<Vec<u8>> {
        self.db
            .get_cf(
                MerkleTreeColumnFamily::LeafIndices,
                &LevelIndex::bin_key(key),
            )
            .unwrap()
    }

    /// Prepares db update
    pub fn pre_save(&mut self, branches: &HashMap<LevelIndex, TreeKey>) {
        for (level_index, tree_key) in branches {
            match self.put_pending_patch.entry(level_index.bin_key()) {
                Entry::Occupied(mut entry) => {
                    *entry.get_mut() = tree_key_to_u8_arr(tree_key);
                }
                Entry::Vacant(entry) => {
                    entry.insert(tree_key_to_u8_arr(tree_key));
                }
            }
        }
    }

    /// Saves current state to db
    pub fn save(&mut self, block_number: u32) -> Result<(), TreeError> {
        if self.put_pending_patch.is_empty() && self.delete_pending_patch.is_empty() {
            return Err(TreeError::EmptyPatch(String::from(
                "Empty pending patch in storage",
            )));
        }
        let mut write_batch = self.db.new_write_batch();
        for (key, value) in &self.put_pending_patch {
            write_batch.put_cf(MerkleTreeColumnFamily::LeafIndices, &key, &value);
        }

        for key in &self.delete_pending_patch {
            write_batch.delete_cf(MerkleTreeColumnFamily::LeafIndices, &key)
        }

        write_batch.put_cf(
            MerkleTreeColumnFamily::Tree,
            BLOCK_NUMBER_KEY,
            &serialize_block_number(block_number),
        );

        // Sync write is not used here intentionally. It somewhat improves write
        // performance. Overall flow is designed in such way that data is
        // committed to state keeper first and, in case of process crash, tree
        // state is recoverable
        self.db
            .write(write_batch)
            .map_err(TreeError::StorageIoError)
    }

    /// Updates mapping between leaf index and its historical first occurrence
    /// and returns it
    ///
    /// note: for simplicity this column family update is done separately from
    /// the main one so column families can become out of sync in the case
    /// of intermediate process crash but after restart state is fully
    /// recoverable
    pub fn process_leaf_indices(
        &mut self,
        storage_logs: &[(usize, (TreeKey, TreeOperation))],
    ) -> Result<Vec<LeafIndices>, TreeError> {
        let mut current_index = self
            .db
            .get_cf(MerkleTreeColumnFamily::LeafIndices, LEAF_INDEX_KEY)
            .map_err(|err| TreeError::StorageIoError(err))?
            .map(|bytes| deserialize_leaf_index(&bytes))
            .unwrap_or(1);

        let mut put_batch = std::mem::take(&mut self.put_pending_patch);
        let mut delete_batch = std::mem::take(&mut self.delete_pending_patch);
        let mut new_writes = HashMap::new();

        let result = self
            .db
            .multi_get_cf(
                MerkleTreeColumnFamily::LeafIndices,
                storage_logs
                    .iter()
                    .map(|(_, (key, _))| serialize_tree_leaf(*key)),
            )
            .into_iter()
            .zip(storage_logs)
            .group_by(|(_, &(block, _))| block)
            .into_iter()
            .map(|(_block, group)| {
                let mut repeated_writes = Vec::new();
                let mut initial_writes = Vec::new();
                let previous_index = current_index;

                let leaf_indices = group
                    .map(|(raw_data, &(_, (leaf, tree_operation)))| {
                        let leaf_index = match (
                            raw_data.expect("failed to fetch leaf index"),
                            tree_operation,
                        ) {
                            // revert of first occurrence
                            (_, TreeOperation::Delete) => {
                                delete_batch.push(serialize_tree_leaf(leaf));
                                current_index -= 1;
                                0
                            }
                            // existing leaf
                            (Some(bytes), TreeOperation::Write { value, .. }) => {
                                let index = deserialize_leaf_index(&bytes);
                                repeated_writes.push(RepeatedStorageWrite {
                                    index,
                                    value: tree_value_to_h256(&value),
                                });
                                index
                            }
                            (Some(bytes), TreeOperation::Read(_)) => deserialize_leaf_index(&bytes),
                            // first occurrence read (noop)
                            (None, TreeOperation::Read(_)) => *new_writes.get(&leaf).unwrap_or(&0),
                            // first occurrence write
                            (None, TreeOperation::Write { value, .. }) => {
                                // Since there can't be 2 logs for the same slot in one block,
                                // we can safely assume that if we have a new write, it was done in
                                // a previous block and thus the new
                                // index is valid.
                                debug!("leaf:{:?}", leaf.clone());
                                if let Some(&index) = new_writes.get(&leaf) {
                                    debug!("index:{:?}", index);
                                    repeated_writes.push(RepeatedStorageWrite {
                                        index,
                                        value: tree_value_to_h256(&value),
                                    });
                                    index
                                } else {
                                    let index = current_index;
                                    put_batch.insert(
                                        serialize_tree_leaf(leaf),
                                        serialize_leaf_index(index),
                                    );
                                    initial_writes.push(InitialStorageWrite {
                                        index,
                                        key: tree_key_to_u256(&leaf),
                                        value: tree_value_to_h256(&value),
                                    });
                                    new_writes.insert(leaf, index);
                                    debug!("current_index:{:?}", current_index);

                                    current_index += 1;
                                    index
                                }
                            }
                        };
                        (leaf, leaf_index)
                    })
                    .collect();

                LeafIndices {
                    leaf_indices,
                    previous_index,
                    initial_writes,
                    repeated_writes,
                    last_index: current_index,
                }
            })
            .collect();

        put_batch.insert(LEAF_INDEX_KEY.to_vec(), serialize_leaf_index(current_index));
        self.put_pending_patch = put_batch;
        self.delete_pending_patch = delete_batch;

        Ok(result)
    }

    /// Fetches high-level metadata about merkle tree state
    pub fn fetch_metadata(&self) -> StoredTreeMetadata {
        // Fetch root hash. It is represented by level index (0, 0).
        let binding = (0, 0.into()).into();
        let keys = vec![&binding];
        let root_hash = self.hashes(keys)[0].clone();
        // let root_hash = self.hashes(vec![&(0, 0.into()).into()])[0].clone();

        let block_number = self
            .db
            .get_cf(MerkleTreeColumnFamily::Tree, BLOCK_NUMBER_KEY)
            .expect("failed to fetch tree metadata")
            .map(|bytes| deserialize_block_number(&bytes))
            .unwrap_or(0);
        if let Some(root_hash) = root_hash {
            let root_hash = u8_arr_to_tree_key(&root_hash);
            return (Some(root_hash), block_number);
        }

        return (None, block_number);
    }
}

/// High level merkle tree metadata
/// Includes root hash and current block number
pub(crate) type StoredTreeMetadata = (Option<ZkHash>, u32);
