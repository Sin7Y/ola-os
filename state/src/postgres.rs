use std::{
    mem,
    sync::{Arc, RwLock},
    time::Instant,
};

use ola_dal::{connection::ConnectionPool, StorageProcessor};
use ola_types::{
    storage::{StorageKey, StorageValue},
    L1BatchNumber, MiniblockNumber, H256,
};
use tokio::{runtime::Handle, sync::mpsc};

use crate::{
    cache::{Cache, CacheValue},
    ReadStorage,
};

type FactoryDepsCache = Cache<H256, Vec<u8>>;

impl CacheValue<H256> for Vec<u8> {
    fn cache_weight(&self) -> u32 {
        self.len().try_into().expect("Cached bytes are too large")
    }
}

type InitialWritesCache = Cache<StorageKey, L1BatchNumber>;

impl CacheValue<StorageKey> for L1BatchNumber {
    fn cache_weight(&self) -> u32 {
        const WEIGHT: usize = mem::size_of::<L1BatchNumber>() + mem::size_of::<StorageKey>();
        WEIGHT as u32
    }
}

impl CacheValue<H256> for StorageValue {
    fn cache_weight(&self) -> u32 {
        // account key + value
        const WEIGHT: usize = mem::size_of::<StorageValue>() + mem::size_of::<H256>();
        WEIGHT as u32
    }
}

#[derive(Debug)]
struct ValuesCacheInner {
    /// Miniblock for which `self.values` are valid. Has the same meaning as `miniblock_number`
    /// in `PostgresStorage` (i.e., the latest sealed miniblock for which storage logs should
    /// be taken into account).
    valid_for: MiniblockNumber,
    values: Cache<H256, StorageValue>,
}

#[derive(Debug, Clone)]
struct ValuesCache(Arc<RwLock<ValuesCacheInner>>);

impl ValuesCache {
    fn new(capacity: u64) -> Self {
        let inner = ValuesCacheInner {
            valid_for: MiniblockNumber(0),
            values: Cache::new("values_cache", capacity),
        };
        Self(Arc::new(RwLock::new(inner)))
    }

    fn valid_for(&self) -> MiniblockNumber {
        self.0.read().expect("value cache is poisoned").valid_for
    }

    /// Gets the cached value for `key` provided that the cache currently holds values
    /// for `miniblock_number`.
    fn get(&self, miniblock_number: MiniblockNumber, key: &StorageKey) -> Option<StorageValue> {
        let lock = self.0.read().expect("values cache is poisoned");
        if lock.valid_for == miniblock_number {
            lock.values.get(&key.hashed_key())
        } else {
            None
        }
    }

    /// Caches `value` for `key`, but only if the cache currently holds values for `miniblock_number`.
    fn insert(&self, miniblock_number: MiniblockNumber, key: StorageKey, value: StorageValue) {
        let lock = self.0.read().expect("values cache is poisoned");
        if lock.valid_for == miniblock_number {
            lock.values.insert(key.hashed_key(), value);
        }
    }

    fn update(
        &self,
        from_miniblock: MiniblockNumber,
        to_miniblock: MiniblockNumber,
        rt_handle: &Handle,
        connection: &mut StorageProcessor<'_>,
    ) {
        const MAX_MINIBLOCKS_LAG: u32 = 5;

        olaos_logs::info!(
            "Updating storage values cache from miniblock {from_miniblock} to {to_miniblock}"
        );

        if to_miniblock.0 - from_miniblock.0 > MAX_MINIBLOCKS_LAG {
            olaos_logs::info!(
                "Storage values cache is too far behind (current miniblock is {from_miniblock}; \
                 requested update to {to_miniblock}); resetting the cache"
            );
            let mut lock = self.0.write().expect("value cache is poisoned");
            assert_eq!(lock.valid_for, from_miniblock);
            lock.valid_for = to_miniblock;
            lock.values.clear();
        } else {
            let stage_started_at = Instant::now();
            let miniblocks = (from_miniblock + 1)..=to_miniblock;
            let modified_keys = rt_handle.block_on(
                connection
                    .storage_web3_dal()
                    .modified_keys_in_miniblocks(miniblocks.clone()),
            );
            let elapsed = stage_started_at.elapsed();
            olaos_logs::info!(
                "Loaded {modified_keys_len} modified storage keys from miniblocks {miniblocks:?}; \
                 took {elapsed:?}",
                modified_keys_len = modified_keys.len()
            );
            let mut lock = self.0.write().expect("value cache is poisoned");
            assert_eq!(lock.valid_for, from_miniblock);
            lock.valid_for = to_miniblock;
            for modified_key in &modified_keys {
                lock.values.remove(modified_key);
            }
            drop(lock);
        }
    }
}

#[derive(Debug, Clone)]
struct ValuesCacheAndUpdater {
    cache: ValuesCache,
    command_sender: mpsc::UnboundedSender<MiniblockNumber>,
}

#[derive(Debug, Clone)]
pub struct PostgresStorageCaches {
    factory_deps: FactoryDepsCache,
    initial_writes: InitialWritesCache,
    negative_initial_writes: InitialWritesCache,
    values: Option<ValuesCacheAndUpdater>,
}

impl PostgresStorageCaches {
    const NEG_INITIAL_WRITES_NAME: &'static str = "negative_initial_writes_cache";

    pub fn new(factory_deps_capacity: u64, initial_writes_capacity: u64) -> Self {
        olaos_logs::info!(
            "Initialized VM execution cache with {factory_deps_capacity}B capacity for factory deps, \
             {initial_writes_capacity}B capacity for initial writes"
        );

        Self {
            factory_deps: FactoryDepsCache::new("factory_deps_cache", factory_deps_capacity),
            initial_writes: InitialWritesCache::new(
                "initial_writes_cache",
                initial_writes_capacity / 2,
            ),
            negative_initial_writes: InitialWritesCache::new(
                Self::NEG_INITIAL_WRITES_NAME,
                initial_writes_capacity / 2,
            ),
            values: None,
        }
    }

    pub fn configure_storage_values_cache(
        &mut self,
        capacity: u64,
        conection_pool: ConnectionPool,
        rt_handle: Handle,
    ) -> impl FnOnce() -> anyhow::Result<()> + Send {
        assert!(capacity > 0, "Storage calues cache mut be positive");
        olaos_logs::info!("Initializing VM storage values cache with {capacity}B capacity");

        let (command_sender, mut command_receiver) = mpsc::unbounded_channel();
        let values_cache = ValuesCache::new(capacity);
        self.values = Some(ValuesCacheAndUpdater {
            cache: values_cache.clone(),
            command_sender,
        });

        move || {
            let mut current_miniblock = values_cache.valid_for();
            while let Some(to_miniblock) = command_receiver.blocking_recv() {
                if to_miniblock <= current_miniblock {
                    continue;
                }
                let mut connection =
                    rt_handle.block_on(conection_pool.access_storage_tagged("value_cache_updater"));
                values_cache.update(current_miniblock, to_miniblock, &rt_handle, &mut connection);
                current_miniblock = to_miniblock;
            }
            Ok(())
        }
    }

    pub fn schedule_values_update(&self, to_miniblock: MiniblockNumber) {
        let values = self
            .values
            .as_ref()
            .expect("`schedule_update()` called without configuring values cache");

        if values.cache.valid_for() < to_miniblock {
            // Filter out no-op updates right away in order to not store lots of them in RAM.
            values
                .command_sender
                .send(to_miniblock)
                .expect("values cache update task failed");
        }
    }
}

#[derive(Debug)]
pub struct PostgresStorage<'a> {
    rt_handle: Handle,
    connection: StorageProcessor<'a>,
    miniblock_number: MiniblockNumber,
    l1_batch_number_for_miniblock: L1BatchNumber,
    pending_l1_batch_number: L1BatchNumber,
    consider_new_l1_batch: bool,
    caches: Option<PostgresStorageCaches>,
}

impl<'a> PostgresStorage<'a> {
    /// Creates a new storage using the specified connection.
    pub fn new(
        rt_handle: Handle,
        mut connection: StorageProcessor<'a>,
        block_number: MiniblockNumber,
        consider_new_l1_batch: bool,
    ) -> PostgresStorage<'a> {
        let resolved = rt_handle
            .block_on(
                connection
                    .storage_web3_dal()
                    .resolve_l1_batch_number_of_miniblock(block_number),
            )
            .expect("Failed resolving L1 batch number for miniblock");

        Self {
            rt_handle,
            connection,
            miniblock_number: block_number,
            l1_batch_number_for_miniblock: resolved.expected_l1_batch(),
            pending_l1_batch_number: resolved.pending_l1_batch,
            consider_new_l1_batch,
            caches: None,
        }
    }

    /// Sets the caches to use with the storage.
    #[must_use]
    pub fn with_caches(self, mut caches: PostgresStorageCaches) -> Self {
        let should_use_values_cache = caches.values.as_ref().map_or(false, |values| {
            self.miniblock_number >= values.cache.valid_for()
        });
        // Since "valid for" only increases with time, if `self.miniblock_number < valid_for`,
        // all cache calls are guaranteed to miss.

        if !should_use_values_cache {
            caches.values = None;
        }

        Self {
            caches: Some(caches),
            ..self
        }
    }

    /// This method is expected to be called for each write that was found in the database, and it decides
    /// whether the change is initial or not. Even if a change is present in the DB, in some cases we would not consider it.
    /// For example, in API we always represent the state at the beginning of an L1 batch, so we discard all the writes
    /// that happened at the same batch or later (for historical `eth_call` requests).
    fn write_counts(&self, write_l1_batch_number: L1BatchNumber) -> bool {
        if self.consider_new_l1_batch {
            self.l1_batch_number_for_miniblock >= write_l1_batch_number
        } else {
            self.l1_batch_number_for_miniblock > write_l1_batch_number
        }
    }

    fn values_cache(&self) -> Option<&ValuesCache> {
        Some(&self.caches.as_ref()?.values.as_ref()?.cache)
    }
}

impl ReadStorage for PostgresStorage<'_> {
    fn read_value(&mut self, &key: &StorageKey) -> StorageValue {
        let values_cache = self.values_cache();
        let cached_value = values_cache.and_then(|cache| cache.get(self.miniblock_number, &key));

        let value = cached_value.unwrap_or_else(|| {
            let mut dal = self.connection.storage_web3_dal();
            let value = self
                .rt_handle
                .block_on(dal.get_historical_value_unchecked(&key, self.miniblock_number))
                .expect("Failed executing `read_value`");
            if let Some(cache) = self.values_cache() {
                cache.insert(self.miniblock_number, key, value);
            }
            value
        });

        value
    }

    fn is_write_initial(&mut self, key: &StorageKey) -> bool {
        let caches = self.caches.as_ref();
        let cached_value = caches.and_then(|caches| caches.initial_writes.get(key));

        if cached_value.is_none() {
            // Write is absent in positive cache, check whether it's present in the negative cache.
            let cached_value = caches.and_then(|caches| caches.negative_initial_writes.get(key));
            if let Some(min_l1_batch_for_initial_write) = cached_value {
                // We know that this slot was certainly not touched before `min_l1_batch_for_initial_write`.
                // Try to use this knowledge to decide if the change is certainly initial.
                // This is based on the hypothetical worst-case scenario, in which the key was
                // written to at the earliest possible L1 batch (i.e., `min_l1_batch_for_initial_write`).
                if !self.write_counts(min_l1_batch_for_initial_write) {
                    return true;
                }
            }
        }

        let l1_batch_number = cached_value.or_else(|| {
            let mut dal = self.connection.storage_web3_dal();
            let value = self
                .rt_handle
                .block_on(dal.get_l1_batch_number_for_initial_write(key))
                .expect("Failed executing `is_write_initial`");

            if let Some(caches) = &self.caches {
                if let Some(l1_batch_number) = value {
                    caches.negative_initial_writes.remove(key);
                    caches.initial_writes.insert(*key, l1_batch_number);
                } else {
                    caches
                        .negative_initial_writes
                        .insert(*key, self.pending_l1_batch_number);
                    // The pending L1 batch might have been sealed since its number was requested from Postgres
                    // in `Self::new()`, so this is a somewhat conservative estimate.
                }
            }
            value
        });

        let contains_key = l1_batch_number.map_or(false, |initial_write_l1_batch_number| {
            self.write_counts(initial_write_l1_batch_number)
        });
        !contains_key
    }

    fn load_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>> {
        let cached_value = self
            .caches
            .as_ref()
            .and_then(|caches| caches.factory_deps.get(&hash));

        let result = cached_value.or_else(|| {
            let mut dal = self.connection.storage_web3_dal();
            let value = self
                .rt_handle
                .block_on(dal.get_factory_dep_unchecked(hash, self.miniblock_number))
                .expect("Failed executing `load_factory_dep`");

            if let Some(caches) = &self.caches {
                // If we receive None, we won't cache it.
                if let Some(dep) = value.clone() {
                    caches.factory_deps.insert(hash, dep);
                }
            };

            value
        });

        result
    }
}
