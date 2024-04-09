use crate::{
    log::{StorageLogKind, WitnessStorageLog},
    patch::{TreePatch, Update, UpdatesBatch},
    storage::Storage,
    tree_config::TreeConfig,
    utils::idx_to_merkle_path,
    TreeError,
};
use olaos_storage::db::RocksDB;
use olavm_core::{
    crypto::ZkHasher,
    trace::trace::HashTrace,
    types::{
        merkle_tree::{
            constant::ROOT_TREE_DEPTH, tree_key_default, tree_key_to_u256, u256_to_tree_key,
            u8_arr_to_tree_key, LeafIndices, LevelIndex, NodeEntry, TreeKey, TreeMetadata,
            TreeOperation, ZkHash,
        },
        proof::StorageLogMetadata,
    },
};

use itertools::Itertools;
use log::{debug, info};
use std::{
    borrow::{Borrow, BorrowMut},
    collections::{hash_map::Entry, HashMap, HashSet},
    iter::once,
    path::Path,
    sync::Arc,
};
use tempfile::TempDir;
use web3::types::U256;

#[derive(Debug)]
pub struct AccountTree {
    pub storage: Storage,
    pub config: TreeConfig<ZkHasher>,
    root_hash: ZkHash,
    block_number: u32,
}

impl AccountTree {
    /// Creates new ZkSyncTree instance
    pub fn new(path: &Path) -> Self {
        let db = RocksDB::new(path);
        let storage = Storage::new(db);
        let config = TreeConfig::new(ZkHasher::default()).expect("TreeConfig new failed");
        let (root_hash, block_number) = storage.fetch_metadata();
        let root_hash = root_hash.unwrap_or_else(|| config.default_root_hash());
        Self {
            storage,
            config,
            root_hash,
            block_number,
        }
    }

    pub fn new_test() -> Self {
        let db_path = TempDir::new().expect("failed get temporary directory for RocksDB");
        let db = RocksDB::new(db_path.path());
        let storage = Storage::new(db);
        let config = TreeConfig::new(ZkHasher::default()).expect("TreeConfig new failed");
        let (root_hash, block_number) = storage.fetch_metadata();
        let root_hash = root_hash.unwrap_or_else(|| config.default_root_hash());
        Self {
            storage,
            config,
            root_hash,
            block_number,
        }
    }

    pub fn new_db_test(db_path: String) -> Self {
        let db = RocksDB::new(Path::new(&db_path));
        let storage = Storage::new(db);
        let config = TreeConfig::new(ZkHasher::default()).expect("TreeConfig new failed");
        let (root_hash, block_number) = storage.fetch_metadata();
        let root_hash = root_hash.unwrap_or_else(|| config.default_root_hash());
        Self {
            storage,
            config,
            root_hash,
            block_number,
        }
    }

    pub fn root_hash(&self) -> ZkHash {
        self.root_hash.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.root_hash == self.config.default_root_hash()
    }

    pub fn block_number(&self) -> u32 {
        self.block_number
    }

    /// Returns current hasher.
    fn hasher(&self) -> &ZkHasher {
        self.config.hasher()
    }

    /// Processes an iterator of block logs, interpreting each nested iterator
    /// as a block. Before going to the next block, the current block will
    /// be sealed. Returns tree metadata for the corresponding blocks.
    ///
    /// - `storage_logs` - an iterator of storage logs for a given block
    pub fn process_block<I>(&mut self, storage_logs: I) -> (Vec<HashTrace>, Option<TreeMetadata>)
    where
        I: IntoIterator,
        I::Item: Borrow<WitnessStorageLog>,
    {
        let (hash_traces, tree_metadata) = self.process_blocks(once(storage_logs));
        (hash_traces, tree_metadata.last().cloned())
    }

    pub fn process_blocks<I>(&mut self, blocks: I) -> (Vec<HashTrace>, Vec<TreeMetadata>)
    where
        I: IntoIterator,
        I::Item: IntoIterator,
        <I::Item as IntoIterator>::Item: Borrow<WitnessStorageLog>,
    {
        // Filter out reading logs and convert writing to the key-value pairs
        let tree_operations: Vec<_> = blocks
            .into_iter()
            .enumerate()
            .map(|(i, logs)| {
                let tree_operations: Vec<_> = logs
                    .into_iter()
                    .map(|log| {
                        let operation = match log.borrow().storage_log.kind {
                            StorageLogKind::RepeatedWrite | StorageLogKind::InitialWrite => {
                                TreeOperation::Write {
                                    value: log.borrow().storage_log.value,
                                    previous_value: log.borrow().previous_value,
                                }
                            }
                            StorageLogKind::Read => {
                                TreeOperation::Read(log.borrow().storage_log.value)
                            }
                        };
                        (log.borrow().storage_log.key, operation)
                    })
                    .collect();

                info!(
                    "Tree processing block {}, with {} logs",
                    self.block_number + i as u32,
                    tree_operations.len(),
                );

                tree_operations
            })
            .collect();

        assert!(
            tree_operations.len() == 1,
            "Tried to process multiple blocks in lightweight mode"
        );

        // Apply all tree operations
        self.apply_updates_batch(tree_operations)
            .expect("Failed to apply logs")
    }

    fn apply_updates_batch(
        &mut self,
        updates_batch: Vec<Vec<(TreeKey, TreeOperation)>>,
    ) -> Result<(Vec<HashTrace>, Vec<TreeMetadata>), TreeError> {
        let total_blocks = updates_batch.len();

        let storage_logs_with_blocks: Vec<_> = updates_batch
            .into_iter()
            .enumerate()
            .flat_map(|(i, logs)| logs.into_iter().map(move |log| (i, log)))
            .collect();

        let total_logs = storage_logs_with_blocks.len();
        debug!(
            "batch total_blocks:{}, total_logs:{}",
            total_blocks, total_logs
        );

        let mut leaf_indices = self
            .storage
            .process_leaf_indices(&storage_logs_with_blocks)?;

        let storage_logs_with_indices: Vec<_> = storage_logs_with_blocks
            .iter()
            .map(|&(block, (key, operation))| {
                let leaf_index = leaf_indices[block].leaf_indices[&key];
                (key, operation, leaf_index)
            })
            .collect();

        let prepared_updates = self.prepare_batch_update(storage_logs_with_indices)?;
        // Update numbers.
        let group_size = prepared_updates
            .updates
            .iter()
            .fold(0, |acc, update| acc + update.1.len());

        let mut updates = prepared_updates.calculate(self.hasher().clone())?;

        let hash_trace: Vec<_> = updates
            .1
            .borrow_mut()
            .lock()
            .map_err(|err| {
                TreeError::MutexLockError(format!("Mutex lock failed in updates with err: {}", err))
            })?
            .clone()
            .into_iter()
            // 1 update for Vec<HashTrace>
            .fold(vec![Vec::new(); group_size], |mut groups, item| {
                groups[item.0].push(item.1);
                groups
            })
            .into_iter()
            .flat_map(|e| e)
            .collect();

        let tree_metadata: Result<Vec<TreeMetadata>, TreeError> = {
            let patch_metadata =
                self.apply_patch(updates.0, &storage_logs_with_blocks, &leaf_indices)?;

            self.root_hash = patch_metadata
                .last()
                .map(|metadata| metadata.root_hash.clone())
                .unwrap_or_else(|| self.root_hash.clone());

            patch_metadata
                .into_iter()
                .zip(storage_logs_with_blocks)
                .group_by(|(_, (block, _))| *block)
                .into_iter()
                .map(|(block, group)| {
                    let LeafIndices {
                        last_index,
                        initial_writes,
                        repeated_writes,
                        previous_index,
                        ..
                    } = std::mem::take(&mut leaf_indices[block]);

                    let metadata: Vec<_> =
                        group.into_iter().map(|(metadata, _)| metadata).collect();
                    let witness_input = bincode::serialize(&(metadata.clone(), previous_index))
                        .expect("witness serialization failed");

                    match metadata.last() {
                        Some(meta) => {
                            let root_hash = meta.root_hash.clone();
                            Ok(TreeMetadata {
                                root_hash,
                                rollup_last_leaf_index: last_index,
                                witness_input,
                                initial_writes,
                                repeated_writes,
                            })
                        }
                        None => Err(TreeError::EmptyPatch(String::from(
                            "Empty matadata in apply_update_batch",
                        ))),
                    }
                })
                .collect::<Vec<Result<TreeMetadata, TreeError>>>()
                .into_iter()
                .collect()
        };

        self.block_number += total_blocks as u32;
        Ok((hash_trace, tree_metadata?))
    }

    /// Prepares all the data which will be needed to calculate new Merkle Trees
    /// without storage access. This method doesn't perform any hashing
    /// operations.
    fn prepare_batch_update<I>(&self, storage_logs: I) -> Result<UpdatesBatch, TreeError>
    where
        I: IntoIterator<Item = (TreeKey, TreeOperation, u64)>,
    {
        let (op_idxs, updates): (Vec<_>, Vec<_>) = storage_logs
            .into_iter()
            .enumerate()
            .map(|(op_idx, (key, op, index))| ((op_idx, key), (key, op, index)))
            .unzip();

        // TODO: parents values in uncles and one leaf instruction value in changes
        let pre_map = self
            .hash_paths_to_leaves(updates.clone().into_iter(), false)
            .zip(op_idxs.clone().into_iter())
            .map(|(parent_nodes, (op_idx, key))| Ok((key, Update::new(op_idx, parent_nodes, key)?)))
            .collect::<Result<Vec<(TreeKey, Update)>, TreeError>>()?
            .into_iter()
            .fold(HashMap::new(), |mut map: HashMap<_, Vec<_>>, (key, op)| {
                match map.entry(key) {
                    Entry::Occupied(mut entry) => entry.get_mut().push(op),
                    Entry::Vacant(entry) => {
                        entry.insert(vec![op]);
                    }
                }
                map
            });
        let pre_map = pre_map
            .iter()
            .map(|e| (tree_key_to_u256(e.0), e.1.clone()))
            .collect();

        // TODO: neighbor values in uncles and one leaf instruction value in changes.
        let map = self
            .hash_paths_to_leaves(updates.into_iter(), true)
            .zip(op_idxs.into_iter())
            .map(|(parent_nodes, (op_idx, key))| Ok((key, Update::new(op_idx, parent_nodes, key)?)))
            .collect::<Result<Vec<(TreeKey, Update)>, TreeError>>()?
            .into_iter()
            .fold(HashMap::new(), |mut map: HashMap<_, Vec<_>>, (key, op)| {
                match map.entry(key) {
                    Entry::Occupied(mut entry) => entry.get_mut().push(op),
                    Entry::Vacant(entry) => {
                        entry.insert(vec![op]);
                    }
                }
                map
            });
        let map = map
            .iter()
            .map(|e| (tree_key_to_u256(e.0), e.1.clone()))
            .collect();
        Ok(UpdatesBatch::new(map, pre_map))
    }

    /// Accepts updated key-value pair and resolves to an iterator which
    /// produces new tree path containing leaf with branch nodes and full
    /// path to the top. This iterator will lazily emit leaf with needed
    /// path to the top node. At the moment of calling given function won't
    /// perform any hashing operation. Note: This method is public so that
    /// it can be used by the data availability repo.
    pub fn hash_paths_to_leaves<'a, 'b: 'a, I>(
        &'a self,
        storage_logs: I,
        nei_flag: bool,
    ) -> impl Iterator<Item = Vec<TreeKey>> + 'a
    where
        I: Iterator<Item = (TreeKey, TreeOperation, u64)> + Clone + 'b,
    {
        let hasher = self.hasher().clone();
        let default_leaf = TreeConfig::empty_leaf(&hasher);

        self.get_leaves_paths(storage_logs.clone().map(|(key, _, _)| key), nei_flag)
            .zip(storage_logs)
            .map(move |(current_path, (_, operation, _))| {
                let hash = match operation {
                    TreeOperation::Write { value, .. } => value,

                    TreeOperation::Delete => default_leaf.clone(),
                    TreeOperation::Read(value) => value,
                };
                current_path
                    .map(|(_, hash)| hash)
                    .chain(once(hash))
                    .collect()
            })
    }

    /// Retrieves leaf with a given key along with full tree path to it.
    /// Note: This method is public so that it can be used by the data
    /// availability repo.
    pub fn get_leaves_paths<'a, 'b: 'a, I>(
        &'a self,
        ids_iter: I,
        nei_flag: bool,
    ) -> impl Iterator<Item = impl DoubleEndedIterator<Item = (TreeKey, TreeKey)> + Clone + 'b> + 'a
    where
        I: Iterator<Item = TreeKey> + Clone + 'a,
    {
        let empty_tree = Arc::new(self.config.empty_tree().to_vec());

        let idxs: HashSet<_> = ids_iter
            .clone()
            .flat_map(|x| idx_to_merkle_path(tree_key_to_u256(&x), nei_flag))
            .collect();

        let branch_map: Arc<HashMap<_, _>> = Arc::new(
            idxs.iter()
                .cloned()
                .zip(self.storage.hashes(idxs.iter()).into_iter())
                .collect(),
        );

        let hash_by_lvl_idx = move |lvl_idx| {
            let value = branch_map
                .get(&lvl_idx)
                .and_then(|x| {
                    if x.is_none() {
                        None
                    } else {
                        Some(u8_arr_to_tree_key(&x.clone().unwrap()))
                    }
                })
                .unwrap_or_else(|| *empty_tree[lvl_idx.0 .0 as usize].hash());

            (u256_to_tree_key(&lvl_idx.0 .1), value)
        };

        ids_iter.into_iter().map(move |idx| {
            idx_to_merkle_path(tree_key_to_u256(&idx), nei_flag).map(hash_by_lvl_idx.clone())
        })
    }

    fn make_node(level: usize, key: TreeKey, node: NodeEntry) -> (LevelIndex, TreeKey) {
        (
            ((ROOT_TREE_DEPTH - level) as u16, tree_key_to_u256(&key)).into(),
            node.into_hash(),
        )
    }

    /// Applies each change from the given patch to the tree.
    fn apply_patch(
        &mut self,
        patch: TreePatch,
        storage_logs: &[(usize, (TreeKey, TreeOperation))],
        leaf_indices: &[LeafIndices],
    ) -> Result<Vec<StorageLogMetadata>, TreeError> {
        let init_branches: HashMap<LevelIndex, TreeKey> = HashMap::new();
        let init_metadata: Vec<StorageLogMetadata> = Vec::new();
        let res = patch.into_iter().zip(storage_logs).fold(
            Ok((init_branches, init_metadata)),
            |acc: Result<(HashMap<LevelIndex, TreeKey>, Vec<StorageLogMetadata>), TreeError>,
             (entries, &(block, (_, storage_log)))| {
                acc.and_then(|(mut branches, mut metadata)| {
                    let leaf_hashed_key = entries
                        .first()
                        .ok_or(TreeError::EmptyPatch(format!("Empty patch apply_patch")))?
                        .0;
                    let leaf_index = leaf_indices
                        .get(block)
                        .ok_or(TreeError::LeafIndexNotFound)?
                        .leaf_indices
                        .get(&leaf_hashed_key)
                        .ok_or(TreeError::LeafIndexNotFound)?
                        .clone();
                    let mut merkle_paths = Vec::with_capacity(ROOT_TREE_DEPTH);

                    branches.extend(entries.into_iter().enumerate().map(|(level, (key, node))| {
                        if let NodeEntry::Branch {
                            right_hash,
                            left_hash,
                            ..
                        } = &node
                        {
                            let witness_hash =
                                if (tree_key_to_u256(&leaf_hashed_key) >> (level - 1)) % 2
                                    == 0.into()
                                {
                                    right_hash
                                } else {
                                    left_hash
                                };
                            merkle_paths.push(witness_hash.clone());
                        }
                        Self::make_node(level, key, node)
                    }));

                    let root_hash = branches
                        .get(&(0, U256::zero()).into())
                        .ok_or(TreeError::EmptyPatch(format!("Empty branches apply_patch")))?
                        .clone();
                    let is_write = !matches!(storage_log, TreeOperation::Read(_));
                    let first_write = is_write
                        && leaf_index
                            >= leaf_indices
                                .get(block)
                                .ok_or(TreeError::LeafIndexNotFound)?
                                .previous_index;
                    let value_written = match storage_log {
                        TreeOperation::Write { value, .. } => value,
                        _ => tree_key_default(),
                    };
                    let value_read = match storage_log {
                        TreeOperation::Write { previous_value, .. } => previous_value,
                        TreeOperation::Read(value) => value,
                        TreeOperation::Delete => tree_key_default(),
                    };
                    let metadata_log = StorageLogMetadata {
                        root_hash,
                        is_write,
                        first_write,
                        merkle_paths,
                        leaf_hashed_key,
                        leaf_enumeration_index: leaf_index,
                        value_written,
                        value_read,
                    };
                    metadata.push(metadata_log);

                    Ok((branches, metadata))
                })
            },
        )?;

        let (branches, metadata) = res;

        // Prepare database changes
        self.storage.pre_save(&branches);
        Ok(metadata)
    }

    pub fn save(&mut self) -> Result<(), TreeError> {
        self.storage.save(self.block_number)
    }
}
