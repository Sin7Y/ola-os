use std::{
    collections::BTreeMap,
    future::Future,
    path::{Path, PathBuf},
    time::Duration,
};

use ola_config::database::MerkleTreeMode;
use ola_dal::StorageProcessor;
use ola_types::{block::L1BatchHeader, L1BatchNumber, StorageKey, H256};
use olaos_health_check::{Health, HealthStatus};
use olaos_merkle_tree::{
    domain::{OlaTree, OlaTreeReader, TreeMetadata},
    recovery::MerkleTreeRecovery,
    Database, Key, NoVersionError, RocksDBWrapper, TreeEntry, TreeEntryWithProof, TreeInstruction,
};
use olaos_storage::{RocksDB, RocksDBOptions, StalledWritesRetries};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use tokio::sync::mpsc;

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
) -> RocksDBWrapper {
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
    multi_get_chunk_size: usize,
) -> RocksDBWrapper {
    olaos_logs::info!(
        "Initializing Merkle tree database at `{path}` with {multi_get_chunk_size} multi-get chunk size, \
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
    let mut db = RocksDBWrapper::from(db);
    db.set_multi_get_chunk_size(multi_get_chunk_size);
    db
}

/// Wrapper around the "main" tree implementation used by [`MetadataCalculator`].
///
/// Async methods provided by this wrapper are not cancel-safe! This is probably not an issue;
/// `OlaTree` is only indirectly available via `MetadataCalculator::run()` entrypoint
/// which consumes `self`. That is, if `MetadataCalculator::run()` is canceled (which we don't currently do,
/// at least not explicitly), all `MetadataCalculator` data including `OlaTree` is discarded.
/// In the unlikely case you get a "`OlaTree` is in inconsistent state" panic,
/// cancellation is most probably the reason.
#[derive(Debug)]
pub(super) struct AsyncTree {
    inner: Option<OlaTree>,
    mode: MerkleTreeMode,
}

impl AsyncTree {
    const INCONSISTENT_MSG: &'static str =
        "`AsyncTree` is in inconsistent state, which could occur after one of its async methods was cancelled";

    pub fn new(db: RocksDBWrapper, mode: MerkleTreeMode) -> Self {
        let tree = match mode {
            MerkleTreeMode::Full => OlaTree::new(db),
            MerkleTreeMode::Lightweight => OlaTree::new_lightweight(db),
        };
        Self {
            inner: Some(tree),
            mode,
        }
    }

    fn as_ref(&self) -> &OlaTree {
        self.inner.as_ref().expect(Self::INCONSISTENT_MSG)
    }

    fn as_mut(&mut self) -> &mut OlaTree {
        self.inner.as_mut().expect(Self::INCONSISTENT_MSG)
    }

    pub fn mode(&self) -> MerkleTreeMode {
        self.mode
    }

    pub fn reader(&self) -> AsyncTreeReader {
        AsyncTreeReader {
            inner: self.inner.as_ref().expect(Self::INCONSISTENT_MSG).reader(),
            mode: self.mode,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }

    pub fn next_l1_batch_number(&self) -> L1BatchNumber {
        self.as_ref().next_l1_batch_number()
    }

    pub fn root_hash(&self) -> H256 {
        self.as_ref().root_hash()
    }

    pub async fn process_l1_batch(
        &mut self,
        storage_logs: Vec<TreeInstruction<StorageKey>>,
    ) -> TreeMetadata {
        let mut tree = self.inner.take().expect(Self::INCONSISTENT_MSG);
        let (tree, metadata) = tokio::task::spawn_blocking(move || {
            let metadata = tree.process_l1_batch(&storage_logs);
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
                tree.save();
                tree
            })
            .await
            .unwrap(),
        );
    }

    pub fn revert_logs(&mut self, last_l1_batch_to_keep: L1BatchNumber) {
        self.as_mut().revert_logs(last_l1_batch_to_keep);
    }
}

/// Async version of [`OlaTreeReader`].
#[derive(Debug, Clone)]
pub(crate) struct AsyncTreeReader {
    inner: OlaTreeReader,
    mode: MerkleTreeMode,
}

impl AsyncTreeReader {
    pub async fn info(self) -> MerkleTreeInfo {
        tokio::task::spawn_blocking(move || MerkleTreeInfo {
            mode: self.mode,
            root_hash: self.inner.root_hash(),
            next_l1_batch_number: self.inner.next_l1_batch_number(),
            leaf_count: self.inner.leaf_count(),
        })
        .await
        .unwrap()
    }

    pub async fn entries_with_proofs(
        self,
        l1_batch_number: L1BatchNumber,
        keys: Vec<Key>,
    ) -> Result<Vec<TreeEntryWithProof>, NoVersionError> {
        tokio::task::spawn_blocking(move || self.inner.entries_with_proofs(l1_batch_number, &keys))
            .await
            .unwrap()
    }
}

/// Async wrapper for [`MerkleTreeRecovery`].
#[derive(Debug, Default)]
pub(super) struct AsyncTreeRecovery {
    inner: Option<MerkleTreeRecovery<RocksDBWrapper>>,
    mode: MerkleTreeMode,
}

impl AsyncTreeRecovery {
    const INCONSISTENT_MSG: &'static str =
        "`AsyncTreeRecovery` is in inconsistent state, which could occur after one of its async methods was cancelled";

    pub fn new(db: RocksDBWrapper, recovered_version: u64, mode: MerkleTreeMode) -> Self {
        Self {
            inner: Some(MerkleTreeRecovery::new(db, recovered_version)),
            mode,
        }
    }

    pub fn recovered_version(&self) -> u64 {
        self.inner
            .as_ref()
            .expect(Self::INCONSISTENT_MSG)
            .recovered_version()
    }

    /// Returns an entry for the specified key.
    pub async fn entries(&mut self, keys: Vec<Key>) -> Vec<TreeEntry> {
        let tree = self.inner.take().expect(Self::INCONSISTENT_MSG);
        let (entry, tree) = tokio::task::spawn_blocking(move || (tree.entries(&keys), tree))
            .await
            .unwrap();
        self.inner = Some(tree);
        entry
    }

    /// Returns the current hash of the tree.
    pub async fn root_hash(&mut self) -> H256 {
        let tree = self.inner.take().expect(Self::INCONSISTENT_MSG);
        let (root_hash, tree) = tokio::task::spawn_blocking(move || (tree.root_hash(), tree))
            .await
            .unwrap();
        self.inner = Some(tree);
        root_hash
    }

    /// Extends the tree with a chunk of recovery entries.
    pub async fn extend(&mut self, entries: Vec<TreeEntry>) {
        let mut tree = self.inner.take().expect(Self::INCONSISTENT_MSG);
        let tree = tokio::task::spawn_blocking(move || {
            tree.extend_random(entries);
            tree
        })
        .await
        .unwrap();

        self.inner = Some(tree);
    }

    pub async fn finalize(self) -> AsyncTree {
        let tree = self.inner.expect(Self::INCONSISTENT_MSG);
        let db = tokio::task::spawn_blocking(|| tree.finalize())
            .await
            .unwrap();
        AsyncTree::new(db, self.mode)
    }
}

/// Tree at any stage of its life cycle.
#[derive(Debug)]
pub(super) enum GenericAsyncTree {
    /// Uninitialized tree.
    Empty {
        db: RocksDBWrapper,
        mode: MerkleTreeMode,
    },
    /// The tree during recovery.
    Recovering(AsyncTreeRecovery),
    /// Tree that is fully recovered and can operate normally.
    Ready(AsyncTree),
}

impl GenericAsyncTree {
    pub async fn new(db: RocksDBWrapper, mode: MerkleTreeMode) -> Self {
        tokio::task::spawn_blocking(move || {
            let Some(manifest) = db.manifest() else {
                return Self::Empty { db, mode };
            };
            if let Some(version) = manifest.recovered_version() {
                Self::Recovering(AsyncTreeRecovery::new(db, version, mode))
            } else {
                Self::Ready(AsyncTree::new(db, mode))
            }
        })
        .await
        .unwrap()
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

    pub fn delay_interval(&self) -> Duration {
        self.delay_interval
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

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) struct L1BatchWithLogs {
    pub header: L1BatchHeader,
    pub storage_logs: Vec<TreeInstruction<StorageKey>>,
}

impl L1BatchWithLogs {
    pub async fn new(
        storage: &mut StorageProcessor<'_>,
        l1_batch_number: L1BatchNumber,
    ) -> Option<Self> {
        let header = storage
            .blocks_dal()
            .get_l1_batch_header(l1_batch_number)
            .await
            .unwrap();

        let protective_reads = storage
            .storage_logs_dedup_dal()
            .get_protective_reads_for_l1_batch(l1_batch_number)
            .await;

        let mut touched_slots = storage
            .storage_logs_dal()
            .get_touched_slots_for_l1_batch(l1_batch_number)
            .await;

        let hashed_keys_for_writes: Vec<_> =
            touched_slots.keys().map(StorageKey::hashed_key).collect();
        let l1_batches_for_initial_writes = storage
            .storage_logs_dal()
            .get_l1_batches_and_indices_for_initial_writes(&hashed_keys_for_writes)
            .await;

        let mut storage_logs = BTreeMap::new();
        for storage_key in protective_reads {
            touched_slots.remove(&storage_key);
            // ^ As per deduplication rules, all keys in `protective_reads` haven't *really* changed
            // in the considered L1 batch. Thus, we can remove them from `touched_slots` in order to simplify
            // their further processing.
            let log = TreeInstruction::Read(storage_key);
            storage_logs.insert(storage_key, log);
        }

        for (storage_key, value) in touched_slots {
            if let Some(&(initial_write_batch_for_key, leaf_index)) =
                l1_batches_for_initial_writes.get(&storage_key.hashed_key())
            {
                // Filter out logs that correspond to deduplicated writes.
                if initial_write_batch_for_key <= l1_batch_number {
                    storage_logs.insert(
                        storage_key,
                        TreeInstruction::write(storage_key, leaf_index, value),
                    );
                }
            }
        }

        Some(Self {
            header,
            storage_logs: storage_logs.into_values().collect(),
        })
    }
}
