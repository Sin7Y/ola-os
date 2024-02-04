use std::{
    collections::HashMap,
    fmt,
    time::{Duration, Instant},
};

use ola_types::{StorageKey, StorageValue, H256};

use crate::{ReadStorage, WriteStorage};

#[derive(Debug, Default, Clone, Copy)]
pub struct StorageViewMetrics {
    /// Estimated byte size of the cache used by the `StorageView`.
    pub cache_size: usize,
    /// Number of read / write ops for which the value was read from the underlying storage.
    pub storage_invocations_missed: usize,
    /// Number of processed read ops.
    pub get_value_storage_invocations: usize,
    /// Number of processed write ops.
    pub set_value_storage_invocations: usize,
    /// Cumulative time spent on reading data from the underlying storage.
    pub time_spent_on_storage_missed: Duration,
    /// Cumulative time spent on all read ops.
    pub time_spent_on_get_value: Duration,
    /// Cumulative time spent on all write ops.
    pub time_spent_on_set_value: Duration,
}

#[derive(Debug)]
pub struct StorageView<S> {
    storage_handle: S,
    // Used for caching and to get the list/count of modified keys
    modified_storage_keys: HashMap<StorageKey, StorageValue>,
    // Used purely for caching
    read_storage_keys: HashMap<StorageKey, StorageValue>,
    // Cache for `contains_key()` checks. The cache is only valid within one L1 batch execution.
    initial_writes_cache: HashMap<StorageKey, bool>,
    metrics: StorageViewMetrics,
}

impl<S: ReadStorage + fmt::Debug> StorageView<S> {
    /// Creates a new storage view based on the underlying storage.
    pub fn new(storage_handle: S) -> Self {
        Self {
            storage_handle,
            modified_storage_keys: HashMap::new(),
            read_storage_keys: HashMap::new(),
            initial_writes_cache: HashMap::new(),
            metrics: StorageViewMetrics::default(),
        }
    }

    /// Creates a storage view with the pre-filled cache of keys read from the storage.
    pub fn new_with_read_keys(
        storage_handle: S,
        read_storage_keys: HashMap<StorageKey, StorageValue>,
    ) -> Self {
        Self {
            read_storage_keys,
            ..Self::new(storage_handle)
        }
    }

    fn get_value_no_log(&mut self, key: &StorageKey) -> StorageValue {
        let started_at = Instant::now();

        let cached_value = self
            .modified_storage_keys
            .get(key)
            .or_else(|| self.read_storage_keys.get(key));
        cached_value.copied().unwrap_or_else(|| {
            let value = self.storage_handle.read_value(key);
            self.read_storage_keys.insert(*key, value);
            self.metrics.time_spent_on_storage_missed += started_at.elapsed();
            self.metrics.storage_invocations_missed += 1;
            value
        })
    }

    /// Unwraps this view, retrieving the read cache. This should be used in tandem with
    /// [`Self::new_with_read_keys()`] to share the read cache across multiple views.
    pub fn into_read_storage_keys(self) -> HashMap<StorageKey, StorageValue> {
        self.read_storage_keys
    }
}

impl<S: ReadStorage + fmt::Debug> ReadStorage for StorageView<S> {
    fn read_value(&mut self, key: &StorageKey) -> StorageValue {
        let started_at = Instant::now();
        self.metrics.get_value_storage_invocations += 1;
        let value = self.get_value_no_log(key);

        olaos_logs::info!(
            "read value {:?} {:?} ({:?}/{:?})",
            key.hashed_key().0,
            value.0,
            key.address(),
            key.key()
        );

        self.metrics.time_spent_on_get_value += started_at.elapsed();
        value
    }

    /// Only keys contained in the underlying storage will return `false`. If a key was
    /// inserted using [`Self::set_value()`], it will still return `true`.
    fn is_write_initial(&mut self, key: &StorageKey) -> bool {
        if let Some(&is_write_initial) = self.initial_writes_cache.get(key) {
            is_write_initial
        } else {
            let is_write_initial = self.storage_handle.is_write_initial(key);
            self.initial_writes_cache.insert(*key, is_write_initial);
            is_write_initial
        }
    }

    fn load_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>> {
        self.storage_handle.load_factory_dep(hash)
    }
}

impl<S: ReadStorage + fmt::Debug> WriteStorage for StorageView<S> {
    fn set_value(&mut self, key: StorageKey, value: StorageValue) -> StorageValue {
        let started_at = Instant::now();
        self.metrics.set_value_storage_invocations += 1;
        let original = self.get_value_no_log(&key);

        olaos_logs::info!(
            "write value {:?} value: {:?} original value: {:?} ({:?}/{:?})",
            key.hashed_key().0,
            value,
            original,
            key.address(),
            key.key()
        );
        self.modified_storage_keys.insert(key, value);
        self.metrics.time_spent_on_set_value += started_at.elapsed();

        original
    }

    fn modified_storage_keys(&self) -> &HashMap<StorageKey, StorageValue> {
        &self.modified_storage_keys
    }

    fn missed_storage_invocations(&self) -> usize {
        self.metrics.storage_invocations_missed
    }
}
