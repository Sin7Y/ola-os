use std::collections::{HashMap, HashSet};

use ola_basic_types::{AccountTreeId, U256};
use ola_utils::u256_to_h256;

use crate::{
    log::{StorageLogQuery, StorageLogQueryType},
    tx::tx_execution_info::DeduplicatedWritesMetrics,
    StorageKey,
};

#[derive(Debug, Clone, Copy, PartialEq)]
struct UpdateItem {
    key: StorageKey,
    is_insertion: bool,
    is_write_initial: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StorageWritesDeduplicator {
    initial_values: HashMap<StorageKey, U256>,
    modified_keys: HashSet<StorageKey>,
    metrics: DeduplicatedWritesMetrics,
}

impl StorageWritesDeduplicator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn metrics(&self) -> DeduplicatedWritesMetrics {
        self.metrics
    }

    pub fn apply<'a, I: IntoIterator<Item = &'a StorageLogQuery>>(&mut self, logs: I) {
        self.process_storage_logs(logs);
    }

    pub fn apply_and_rollback<'a, I: IntoIterator<Item = &'a StorageLogQuery>>(
        &mut self,
        logs: I,
    ) -> DeduplicatedWritesMetrics {
        let updates = self.process_storage_logs(logs);
        let metrics = self.metrics;
        self.rollback(updates);
        metrics
    }

    pub fn apply_on_empty_state<'a, I: IntoIterator<Item = &'a StorageLogQuery>>(
        logs: I,
    ) -> DeduplicatedWritesMetrics {
        let mut deduplicator = Self::new();
        deduplicator.apply(logs);
        deduplicator.metrics
    }

    fn process_storage_logs<'a, I: IntoIterator<Item = &'a StorageLogQuery>>(
        &mut self,
        logs: I,
    ) -> Vec<UpdateItem> {
        let mut updates = Vec::new();
        for log in logs.into_iter().filter(|log| log.log_query.rw_flag) {
            let key = StorageKey::new(
                AccountTreeId::new(log.log_query.address),
                u256_to_h256(log.log_query.key),
            );
            let initial_value = *self
                .initial_values
                .entry(key)
                .or_insert(log.log_query.read_value);

            let was_key_modified = self.modified_keys.get(&key).is_some();
            let is_key_modified = if log.log_query.rollback {
                initial_value != log.log_query.read_value
            } else {
                initial_value != log.log_query.written_value
            };

            let is_write_initial = log.log_type == StorageLogQueryType::InitialWrite;
            let field_to_change = if is_write_initial {
                &mut self.metrics.initial_storage_writes
            } else {
                &mut self.metrics.repeated_storage_writes
            };

            match (was_key_modified, is_key_modified) {
                (true, false) => {
                    self.modified_keys.remove(&key);
                    *field_to_change -= 1;
                    updates.push(UpdateItem {
                        key,
                        is_insertion: false,
                        is_write_initial,
                    });
                }
                (false, true) => {
                    self.modified_keys.insert(key);
                    *field_to_change += 1;
                    updates.push(UpdateItem {
                        key,
                        is_insertion: true,
                        is_write_initial,
                    });
                }
                _ => {}
            }
        }
        updates
    }

    fn rollback(&mut self, updates: Vec<UpdateItem>) {
        for item in updates.into_iter().rev() {
            let field_to_change = if item.is_write_initial {
                &mut self.metrics.initial_storage_writes
            } else {
                &mut self.metrics.repeated_storage_writes
            };

            if item.is_insertion {
                self.modified_keys.remove(&item.key);
                *field_to_change -= 1;
            } else {
                self.modified_keys.insert(item.key);
                *field_to_change += 1;
            }
        }
    }
}
