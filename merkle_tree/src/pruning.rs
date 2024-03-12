//! Tree pruning logic.

use std::{fmt, sync::mpsc, time::Duration};

use crate::storage::{PruneDatabase, PrunePatchSet};

/// Handle for a [`MerkleTreePruner`] allowing to abort its operation.
///
/// The pruner is aborted once the handle is dropped.
#[must_use = "Pruner is aborted once handle is dropped"]
#[derive(Debug)]
pub struct MerkleTreePrunerHandle {
    aborted_sender: mpsc::Sender<()>,
}

impl MerkleTreePrunerHandle {
    /// Aborts the pruner that this handle is attached to. If the pruner has already terminated
    /// (e.g., due to a panic), this is a no-op.
    pub fn abort(self) {
        self.aborted_sender.send(()).ok();
    }
}

/// Component responsible for Merkle tree pruning, i.e. removing nodes not referenced by new versions
/// of the tree. A pruner should be instantiated using a [`Clone`] of the tree database, possibly
/// configured and then [`run()`](Self::run()) on its own thread. [`MerkleTreePrunerHandle`] provides
/// a way to gracefully shut down the pruner.
///
/// # Implementation details
///
/// Pruning works by recording stale node keys each time the Merkle tree is updated; in RocksDB,
/// stale keys are recorded in a separate column family. A pruner takes stale keys that were produced
/// by a certain range of tree versions, and removes the corresponding nodes from the tree
/// (in RocksDB, this uses simple pointwise `delete_cf()` operations). The range of versions
/// depends on pruning policies; for now, it's "remove versions older than `latest_version - N`",
/// where `N` is a configurable number set when the pruner [is created](Self::new()).
pub struct MerkleTreePruner<DB> {
    db: DB,
    past_versions_to_keep: u64,
    target_pruned_key_count: usize,
    poll_interval: Duration,
    aborted_receiver: mpsc::Receiver<()>,
}

impl<DB> fmt::Debug for MerkleTreePruner<DB> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MerkleTreePruner")
            .field("past_versions_to_keep", &self.past_versions_to_keep)
            .field("target_pruned_key_count", &self.target_pruned_key_count)
            .field("poll_interval", &self.poll_interval)
            .finish_non_exhaustive()
    }
}

impl<DB: PruneDatabase> MerkleTreePruner<DB> {
    /// Creates a pruner with the specified database and the number of past tree versions to keep.
    /// E.g., 0 means keeping only the latest version.
    ///
    /// # Return value
    ///
    /// Returns the created pruner and a handle to it. *The pruner will be aborted when its handle
    /// is dropped.*
    pub fn new(db: DB, past_versions_to_keep: u64) -> (Self, MerkleTreePrunerHandle) {
        let (aborted_sender, aborted_receiver) = mpsc::channel();
        let handle = MerkleTreePrunerHandle { aborted_sender };
        let this = Self {
            db,
            past_versions_to_keep,
            target_pruned_key_count: 500_000,
            poll_interval: Duration::from_secs(60),
            aborted_receiver,
        };
        (this, handle)
    }

    /// Sets the target number of stale keys pruned on a single iteration. This limits the size of
    /// a produced RocksDB `WriteBatch` and the RAM consumption of the pruner. At the same time,
    /// larger values can lead to more efficient RocksDB compaction.
    ///
    /// Reasonable values are order of 100k – 1M. The default value is 500k.
    pub fn set_target_pruned_key_count(&mut self, count: usize) {
        self.target_pruned_key_count = count;
    }

    /// Sets the sleep duration when the pruner cannot progress. This time should be enough
    /// for the tree to produce enough stale keys.
    ///
    /// The default value is 60 seconds.
    pub fn set_poll_interval(&mut self, poll_interval: Duration) {
        self.poll_interval = poll_interval;
    }

    fn target_retained_version(&self) -> Option<u64> {
        let manifest = self.db.manifest()?;
        let latest_version = manifest.version_count.checked_sub(1)?;
        latest_version.checked_sub(self.past_versions_to_keep)
    }

    #[doc(hidden)] // Used in integration tests; logically private
    #[allow(clippy::range_plus_one)] // exclusive range is required by `PrunePatchSet` constructor
    pub fn run_once(&mut self) -> Option<bool> {
        let target_retained_version = self.target_retained_version()?;
        let min_stale_key_version = self.db.min_stale_key_version()?;
        let stale_key_new_versions = min_stale_key_version..=target_retained_version;

        let mut pruned_keys = vec![];
        let mut max_stale_key_version = min_stale_key_version;
        for version in stale_key_new_versions {
            max_stale_key_version = version;
            pruned_keys.extend_from_slice(&self.db.stale_keys(version));
            if pruned_keys.len() >= self.target_pruned_key_count {
                break;
            }
        }

        if pruned_keys.is_empty() {
            return None;
        }
        let deleted_stale_key_versions = min_stale_key_version..(max_stale_key_version + 1);

        let patch = PrunePatchSet::new(pruned_keys, deleted_stale_key_versions.clone());
        self.db.prune(patch);
        Some(target_retained_version + 1 > deleted_stale_key_versions.end)
    }

    /// Runs this pruner indefinitely until it is aborted by dropping its handle.
    pub fn run(mut self) {
        olaos_logs::info!("Started Merkle tree pruner {self:?}");
        loop {
            let timeout = if let Some(has_more_work) = self.run_once() {
                if has_more_work {
                    Duration::ZERO
                } else {
                    self.poll_interval
                }
            } else {
                olaos_logs::debug!("No pruning required per specified policies; waiting");
                self.poll_interval
            };

            match self.aborted_receiver.recv_timeout(timeout) {
                Ok(()) => break, // Abort was requested
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    olaos_logs::warn!(
                        "Pruner handle is dropped without calling `abort()`; exiting"
                    );
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // The pruner handle is alive and wasn't used to abort the pruner.
                }
            }
        }
    }
}
