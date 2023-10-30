use std::collections::BTreeMap;

use ola_dal::StorageProcessor;
use ola_types::{block::L1BatchHeader, log::StorageLog, L1BatchNumber, H256};

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