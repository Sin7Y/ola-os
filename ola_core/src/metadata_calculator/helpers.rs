use std::{
    collections::BTreeMap,
    future::Future,
    mem,
    path::{Path, PathBuf},
    time::Duration,
};
#[cfg(test)]
use tokio::sync::mpsc;
use tempfile::TempDir;

use ola_dal::StorageProcessor;
use ola_types::{block::{L1BatchHeader, WitnessBlockWithLogs}, L1BatchNumber, H256, storage::StorageKey, log::{StorageLog, WitnessStorageLog}};
use olavm_core::{
    merkle_tree::{
        tree::AccountTree, log::{WitnessStorageLog as OlavmWitnessStorageLog, StorageLog as OlavmStorageLog}
    }, storage::db::{RocksDB, Database},
    types::merkle_tree::TreeMetadata,
};

#[derive(Debug, Default)]
pub struct AsyncTree(Option<AccountTree>);

impl AsyncTree {
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
        let storage_logs: Vec<OlavmWitnessStorageLog> = storage_logs.iter().map(|sl| sl.to_olavm_type()).collect();
        let metadata = tree.process_block(storage_logs);
        metadata.1
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

    // #[cfg_attr(not(test), allow(unused))] // `tree` is only used in test mode
    // pub fn wait(&self, tree: &AsyncTree) -> impl Future<Output = ()> {
    //     #[cfg(test)]
    //     self.delay_notifier
    //         .send((tree.next_l1_batch_number(), tree.root_hash()))
    //         .ok();
    //     tokio::time::sleep(self.delay_interval)
    // }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) struct L1BatchWithLogs {
    pub header: L1BatchHeader,
    pub storage_logs: Vec<StorageLog>,
}

impl L1BatchWithLogs {
    pub async fn new(
        storage: &mut StorageProcessor<'_>,
        l1_batch_number: L1BatchNumber,
    ) -> Option<Self> {
        olaos_logs::debug!("Loading storage logs data for L1 batch #{l1_batch_number}");

        let header = storage
            .blocks_dal()
            .get_l1_batch_header(l1_batch_number)
            .await?;

        let protective_reads = storage
            .storage_logs_dedup_dal()
            .get_protective_reads_for_l1_batch(l1_batch_number)
            .await;

        let mut touched_slots = storage
            .storage_logs_dal()
            .get_touched_slots_for_l1_batch(l1_batch_number)
            .await;

        let mut storage_logs = BTreeMap::new();
        for storage_key in protective_reads {
            touched_slots.remove(&storage_key);
            // ^ As per deduplication rules, all keys in `protective_reads` haven't *really* changed
            // in the considered L1 batch. Thus, we can remove them from `touched_slots` in order to simplify
            // their further processing.

            let log = StorageLog::new_read_log(storage_key, H256::zero());
            // ^ The tree doesn't use the read value, so we set it to zero.
            storage_logs.insert(storage_key, log);
        }
        olaos_logs::debug!(
            "Made touched slots disjoint with protective reads; remaining touched slots: {}",
            touched_slots.len()
        );

        // We don't want to update the tree with zero values which were never written to per storage log
        // deduplication rules. If we write such values to the tree, it'd result in bogus tree hashes because
        // new (bogus) leaf indices would be allocated for them. To filter out those values, it's sufficient
        // to check when a `storage_key` was first written per `initial_writes` table. If this never occurred
        // or occurred after the considered `l1_batch_number`, this means that the write must be ignored.
        //
        // Note that this approach doesn't filter out no-op writes of the same value, but this is fine;
        // since no new leaf indices are allocated in the tree for them, such writes are no-op on the tree side as well.
        let hashed_keys_for_zero_values: Vec<_> = touched_slots
            .iter()
            .filter_map(|(key, value)| {
                // Only zero values are worth checking for initial writes; non-zero values are always
                // written per deduplication rules.
                value.is_zero().then(|| key.hashed_key())
            })
            .collect();

        let l1_batches_for_initial_writes = storage
            .storage_logs_dal()
            .get_l1_batches_for_initial_writes(&hashed_keys_for_zero_values)
            .await;

        for (storage_key, value) in touched_slots {
            let write_matters = if value.is_zero() {
                let initial_write_batch_for_key =
                    l1_batches_for_initial_writes.get(&storage_key.hashed_key());
                initial_write_batch_for_key.map_or(false, |&number| number <= l1_batch_number)
            } else {
                true
            };

            if write_matters {
                storage_logs.insert(storage_key, StorageLog::new_write_log(storage_key, value));
            }
        }

        Some(Self {
            header,
            storage_logs: storage_logs.into_values().collect(),
        })
    }
}

pub(crate) async fn get_logs_for_l1_batch(
    storage: &mut StorageProcessor<'_>,
    l1_batch_number: L1BatchNumber,
) -> Option<WitnessBlockWithLogs> {
    let header = storage.blocks_dal().get_l1_batch_header(l1_batch_number).await.unwrap();

    // `BTreeMap` is used because tree needs to process slots in lexicographical order.
    let mut storage_logs: BTreeMap<StorageKey, WitnessStorageLog> = BTreeMap::new();

    let protective_reads = storage
        .storage_logs_dedup_dal()
        .get_protective_reads_for_l1_batch(l1_batch_number).await;
    let touched_slots = storage
        .storage_logs_dal()
        .get_touched_slots_for_l1_batch(l1_batch_number).await;

    let hashed_keys: Vec<_> = protective_reads
        .iter()
        .chain(touched_slots.keys())
        .map(StorageKey::hashed_key)
        .collect();
    let previous_values = storage
        .storage_logs_dal()
        .get_previous_storage_values(&hashed_keys, l1_batch_number).await;

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
        let previous_value = previous_values[&storage_key.hashed_key()].unwrap();
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