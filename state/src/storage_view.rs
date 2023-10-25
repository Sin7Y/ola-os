use std::{collections::HashMap, fmt, time::Duration};

use ola_types::{StorageKey, StorageValue};

use crate::ReadStorage;

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
}
