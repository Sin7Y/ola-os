use std::{
    mem,
    sync::{Arc, RwLock},
};

use ola_dal::{connection::ConnectionPool, StorageProcessor};
use ola_types::{
    storage::{StorageKey, StorageValue},
    L1BatchNumber, MiniblockNumber, H256,
};
use tokio::{runtime::Handle, sync::mpsc};

use crate::cache::{Cache, CacheValue};

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
        const WEIGHT: usize = mem::size_of::<StorageValue>() + mem::size_of::<H256>();
        WEIGHT as u32
    }
}

#[derive(Debug)]
struct ValuesCacheInner {
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

    fn update(
        &self,
        from_miniblock: MiniblockNumber,
        to_miniblock: MiniblockNumber,
        rt_handle: &Handle,
        connection: &mut StorageProcessor<'_>,
    ) {
        const MAX_MINIBLOCKS_LAG: u64 = 5;

        if to_miniblock.0 - from_miniblock.0 > MAX_MINIBLOCKS_LAG {
            let mut lock = self.0.write().expect("value cache is poisoned");
            assert_eq!(lock.valid_for, from_miniblock);
            lock.valid_for = to_miniblock;
            lock.values.clear();
        } else {
            let miniblocks = (from_miniblock + 1)..=to_miniblock;
            let modified_keys = rt_handle.block_on(
                connection
                    .storage_web3_dal()
                    .modified_keys_in_miniblocks(miniblocks.clone()),
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
    ) -> impl FnOnce() + Send {
        assert!(capacity > 0, "Storage calues cache mut be positive");
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
        }
    }
}
