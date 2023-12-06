use serde::Serialize;
use std::{
    borrow::{Borrow, BorrowMut},
    collections::BTreeMap,
    future::Future,
    mem,
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::TempDir;
#[cfg(test)]
use tokio::sync::mpsc;
use tracing::debug;

use ola_dal::StorageProcessor;
use ola_types::{
    block::{L1BatchHeader, WitnessBlockWithLogs},
    log::{StorageLog, WitnessStorageLog},
    storage::{log::StorageLogKind, StorageKey},
    L1BatchNumber, H256,
};
use olaos_health_check::{Health, HealthStatus};
use olavm_core::{
    merkle_tree::{
        log::{StorageLog as OlavmStorageLog, WitnessStorageLog as OlavmWitnessStorageLog},
        tree::AccountTree,
    },
    storage::db::{Database, RocksDB},
    types::merkle_tree::{tree_key_to_h256, TreeMetadata},
};

#[derive(Debug, Serialize)]
pub(super) struct TreeHealthCheckDetails {
    pub next_l1_batch_to_seal: L1BatchNumber,
}

impl From<TreeHealthCheckDetails> for Health {
    fn from(details: TreeHealthCheckDetails) -> Self {
        Self::from(HealthStatus::Ready).with_details(details)
    }
}

#[derive(Debug, Default)]
pub struct AsyncTree(Option<AccountTree>);

impl AsyncTree {
    const INCONSISTENT_MSG: &'static str =
        "`AccountTree` is in inconsistent state, which could occur after one of its blocking futures was cancelled";

    pub async fn new(
        db_path: PathBuf,
        multi_get_chunk_size: usize,
        block_cache_capacity: usize,
    ) -> Self {
        let mut tree = tokio::task::spawn_blocking(move || {
            let db = Self::create_db(&db_path);
            AccountTree::new(db)
        })
        .await
        .unwrap();

        // tree.set_multi_get_chunk_size(multi_get_chunk_size);
        Self(Some(tree))
    }

    fn create_db(path: &Path) -> RocksDB {
        RocksDB::new(Database::MerkleTree, path, true)
    }

    pub fn process_genesis_batch(storage_logs: &[WitnessStorageLog]) -> Option<TreeMetadata> {
        let temp_dir = TempDir::new().expect("failed get temporary directory for RocksDB");
        let db = RocksDB::new(Database::MerkleTree, temp_dir.as_ref(), false);
        let mut tree = AccountTree::new(db);
        let storage_logs: Vec<OlavmWitnessStorageLog> =
            storage_logs.iter().map(|sl| sl.to_olavm_type()).collect();
        let metadata = tree.process_block(storage_logs);
        metadata.1
    }

    pub async fn process_block(&mut self, storage_logs: &[WitnessStorageLog]) -> TreeMetadata {
        let block = Self::filter_block_logs(storage_logs);
        let storage_logs: Vec<OlavmWitnessStorageLog> =
            block.map(|sl| sl.to_olavm_type()).collect();
        let mut tree = mem::take(self);
        let (tree, metadata) = tokio::task::spawn_blocking(move || {
            let metadata = tree.as_mut().process_block(storage_logs);
            (tree, metadata)
        })
        .await
        .unwrap();

        *self = tree;
        metadata.1.unwrap()
    }

    pub async fn process_blocks<'a>(
        &mut self,
        blocks: impl Iterator<Item = &'a [WitnessStorageLog]>,
    ) -> Vec<TreeMetadata> {
        let blocks = blocks.map(|logs| Self::filter_block_logs(logs));
        let storage_logs: Vec<Vec<OlavmWitnessStorageLog>> = blocks
            .map(|sl| sl.map(|l| l.to_olavm_type()).collect())
            .collect();
        let mut tree = mem::take(self);
        let (tree, metadata) = tokio::task::spawn_blocking(move || {
            let metadata = tree.as_mut().process_blocks(storage_logs);
            (tree, metadata)
        })
        .await
        .unwrap();

        *self = tree;
        metadata.1
    }

    pub async fn save(&mut self) {
        let mut tree = mem::take(self);
        *self = tokio::task::spawn_blocking(|| {
            let _ = tree.as_mut().save();
            tree
        })
        .await
        .unwrap();
    }

    fn filter_block_logs(
        logs: &[WitnessStorageLog],
    ) -> impl Iterator<Item = &WitnessStorageLog> + '_ {
        logs.iter()
            .filter(move |log| log.storage_log.kind == StorageLogKind::Write)
    }

    fn as_ref(&self) -> &AccountTree {
        self.0.as_ref().expect(Self::INCONSISTENT_MSG)
    }

    fn as_mut(&mut self) -> &mut AccountTree {
        self.0.as_mut().expect(Self::INCONSISTENT_MSG)
    }

    pub fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }

    pub fn block_number(&self) -> u32 {
        self.as_ref().block_number()
    }

    pub fn root_hash(&self) -> H256 {
        tree_key_to_h256(&self.as_ref().root_hash())
    }
}

/// Component implementing the delay policy in [`MetadataCalculator`] when there are no
/// L1 batches to seal.
#[derive(Debug, Clone)]
pub(super) struct Delayer {
    delay_interval: Duration,
    // Notifies the tests about the next L1 batch number and tree root hash when the calculator
    // runs out of L1 batches to process. (Since RocksDB is exclusive, we cannot just create
    // another instance to check these params on the test side without stopping the calc.)
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
            .send((L1BatchNumber(tree.block_number()), tree.root_hash()))
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
        .await
        .unwrap();

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
        let previous_value = previous_values[&storage_key.hashed_key()].unwrap();
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
