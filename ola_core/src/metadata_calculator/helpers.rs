use std::{
    collections::BTreeMap,
    future::Future,
    path::{Path, PathBuf},
    time::Duration,
};

use merkle_tree2::{storage::MerkleTreeColumnFamily, tree::AccountTree};
use ola_config::database::MerkleTreeMode;
use ola_dal::StorageProcessor;
use ola_types::{
    block::WitnessBlockWithLogs,
    log::{StorageLog, WitnessStorageLog},
    merkle_tree::{tree_key_to_h256, TreeMetadata},
    L1BatchNumber, StorageKey, H256,
};
use olaos_health_check::{Health, HealthStatus};
use olaos_storage::{RocksDB, RocksDBOptions, StalledWritesRetries};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use tokio::sync::mpsc;

#[derive(Debug, Serialize)]
pub(super) struct TreeHealthCheckDetails {
    pub next_l1_batch_to_seal: L1BatchNumber,
}

impl From<TreeHealthCheckDetails> for Health {
    fn from(details: TreeHealthCheckDetails) -> Self {
        Self::from(HealthStatus::Ready).with_details(details)
    }
}
/// General information about the Merkle tree.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MerkleTreeInfo {
    pub mode: MerkleTreeMode,
    pub root_hash: H256,
    pub next_l1_batch_number: L1BatchNumber,
    pub leaf_count: u64,
}

impl From<MerkleTreeInfo> for Health {
    fn from(tree_info: MerkleTreeInfo) -> Self {
        Self::from(HealthStatus::Ready).with_details(tree_info)
    }
}

/// Creates a RocksDB wrapper with the specified params.
pub(super) async fn create_db(
    path: PathBuf,
    block_cache_capacity: usize,
    memtable_capacity: usize,
    stalled_writes_timeout: Duration,
    multi_get_chunk_size: usize,
) -> RocksDB<MerkleTreeColumnFamily> {
    tokio::task::spawn_blocking(move || {
        create_db_sync(
            &path,
            block_cache_capacity,
            memtable_capacity,
            stalled_writes_timeout,
            multi_get_chunk_size,
        )
    })
    .await
    .unwrap()
}

fn create_db_sync(
    path: &Path,
    block_cache_capacity: usize,
    memtable_capacity: usize,
    stalled_writes_timeout: Duration,
    _multi_get_chunk_size: usize,
) -> RocksDB<MerkleTreeColumnFamily> {
    olaos_logs::info!(
        "Initializing Merkle tree database at `{path}` with {_multi_get_chunk_size} multi-get chunk size, \
         {block_cache_capacity}B block cache, {memtable_capacity}B memtable capacity, \
         {stalled_writes_timeout:?} stalled writes timeout",
        path = path.display()
    );

    let mut db = RocksDB::with_options(
        path,
        RocksDBOptions {
            block_cache_capacity: Some(block_cache_capacity),
            large_memtable_capacity: Some(memtable_capacity),
            stalled_writes_retries: StalledWritesRetries::new(stalled_writes_timeout),
        },
    );
    if cfg!(test) {
        // We need sync writes for the unit tests to execute reliably. With the default config,
        // some writes to RocksDB may occur, but not be visible to the test code.
        db = db.with_sync_writes();
    }
    db
}

#[derive(Debug)]
pub(super) struct AsyncTree {
    inner: Option<AccountTree>,
}

impl AsyncTree {
    const INCONSISTENT_MSG: &'static str =
        "`AsyncTree` is in inconsistent state, which could occur after one of its async methods was cancelled";

    pub fn new(db: RocksDB<MerkleTreeColumnFamily>) -> Self {
        let tree = AccountTree::new_with_db(db);
        Self {
            inner: Some(tree),
        }
    }

    fn as_ref(&self) -> &AccountTree {
        self.inner.as_ref().expect(Self::INCONSISTENT_MSG)
    }

    pub fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }

    pub fn next_l1_batch_number(&self) -> L1BatchNumber {
        self.as_ref().block_number().into()
    }

    pub fn root_hash(&self) -> H256 {
        tree_key_to_h256(&self.as_ref().root_hash())
    }

    pub async fn process_l1_batch(&mut self, storage_logs: Vec<WitnessStorageLog>) -> TreeMetadata {
        let mut tree = self.inner.take().expect(Self::INCONSISTENT_MSG);
        let (tree, metadata) = tokio::task::spawn_blocking(move || {
            let metadata = tree.process_block(&storage_logs);
            (tree, metadata)
        })
        .await
        .unwrap();

        self.inner = Some(tree);
        metadata
    }

    pub async fn save(&mut self) {
        let mut tree = self.inner.take().expect(Self::INCONSISTENT_MSG);
        self.inner = Some(
            tokio::task::spawn_blocking(|| {
                let _ = tree.save();
                tree
            })
            .await
            .unwrap(),
        );
    }
}

/// Component implementing the delay policy in [`MetadataCalculator`] when there are no
/// L1 batches to seal.
#[derive(Debug, Clone)]
pub(super) struct Delayer {
    delay_interval: Duration,
    // Notifies the tests about the next L1 batch number and tree root hash when the calculator
    // runs out of L1 batches to process. (Since RocksDB is exclusive, we cannot just create
    // another instance to check these params on the test side without stopping the calculation.)
    #[cfg(test)]
    pub delay_notifier: mpsc::UnboundedSender<(L1BatchNumber, H256)>,
}

impl Delayer {
    pub fn new(delay_interval: Duration) -> Self {
        Self {
            delay_interval,
            #[cfg(test)]
            delay_notifier: mpsc::unbounded_channel().0,
        }
    }

    #[cfg_attr(not(test), allow(unused))] // `tree` is only used in test mode
    pub fn wait(&self, tree: &AsyncTree) -> impl Future<Output = ()> {
        #[cfg(test)]
        self.delay_notifier
            .send((tree.next_l1_batch_number(), tree.root_hash()))
            .ok();
        tokio::time::sleep(self.delay_interval)
    }
}

pub(crate) async fn get_logs_for_l1_batch(
    storage: &mut StorageProcessor<'_>,
    l1_batch_number: L1BatchNumber,
) -> Option<WitnessBlockWithLogs> {
    let header = storage
        .blocks_dal()
        .get_l1_batch_header(l1_batch_number)
        .await?;

    // `BTreeMap` is used because tree needs to process slots in lexicographical order.
    let mut storage_logs: BTreeMap<StorageKey, WitnessStorageLog> = BTreeMap::new();

    let protective_reads = storage
        .storage_logs_dedup_dal()
        .get_protective_reads_for_l1_batch(l1_batch_number)
        .await;
    let touched_slots = storage
        .storage_logs_dal()
        .get_touched_slots_for_l1_batch(l1_batch_number)
        .await;

    let hashed_keys: Vec<_> = protective_reads
        .iter()
        .chain(touched_slots.keys())
        .map(StorageKey::hashed_key)
        .collect();
    let previous_values = storage
        .storage_logs_dal()
        .get_previous_storage_values(&hashed_keys, l1_batch_number)
        .await;

    for storage_key in protective_reads {
        let previous_value = previous_values[&storage_key.hashed_key()].unwrap_or_default();
        // Sanity check: value must not change for slots that require protective reads.
        if let Some(value) = touched_slots.get(&storage_key) {
            assert_eq!(
                previous_value, *value,
                "Value was changed for slot that requires protective read"
            );
        }

        storage_logs.insert(
            storage_key,
            WitnessStorageLog {
                storage_log: StorageLog::new_read_log(storage_key, previous_value),
                previous_value,
            },
        );
    }

    for (storage_key, value) in touched_slots {
        let previous_value = previous_values[&storage_key.hashed_key()].unwrap_or_default();
        if previous_value != value {
            storage_logs.insert(
                storage_key,
                WitnessStorageLog {
                    storage_log: StorageLog::new_write_log(storage_key, value),
                    previous_value,
                },
            );
        }
    }

    Some(WitnessBlockWithLogs {
        header,
        storage_logs: storage_logs.into_values().collect(),
    })
}
