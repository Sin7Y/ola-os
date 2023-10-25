use std::{
    collections::HashSet,
    fmt,
    marker::PhantomData,
    path::Path,
    sync::{Condvar, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use rocksdb::{
    properties, BlockBasedOptions, Cache, ColumnFamily, ColumnFamilyDescriptor, Options,
    WriteOptions, DB,
};

use crate::metrics::{describe_metrics, WriteMetrics};

static ROCKSDB_INSTANCE_COUNTER: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

pub trait NamedColumnFamily: 'static + Copy {
    /// Name of the database. Used in metrics reporting.
    const DB_NAME: &'static str;
    /// Lists all column families in the database.
    const ALL: &'static [Self];
    /// Names a column family to access it in `RocksDB`. Also used in metrics reporting.
    fn name(&self) -> &'static str;
}

#[must_use = "Batch should be written to DB"]
pub struct WriteBatch<'a, CF> {
    inner: rocksdb::WriteBatch,
    db: &'a RocksDB<CF>,
}

impl<CF: NamedColumnFamily> WriteBatch<'_, CF> {
    pub fn put_cf(&mut self, cf: CF, key: &[u8], value: &[u8]) {
        let cf = self.db.column_family(cf);
        self.inner.put_cf(cf, key, value);
    }

    pub fn delete_cf(&mut self, cf: CF, key: &[u8]) {
        let cf = self.db.column_family(cf);
        self.inner.delete_cf(cf, key);
    }

    pub fn delete_range_cf(&mut self, cf: CF, keys: std::ops::Range<&[u8]>) {
        let cf = self.db.column_family(cf);
        self.inner.delete_range_cf(cf, keys.start, keys.end);
    }
}

#[derive(Debug)]
pub struct RocksDB<CF> {
    db: DB,
    sync_writes: bool,
    sizes_reported_at: Mutex<Option<Instant>>,
    _registry_entry: RegistryEntry,
    _cf: PhantomData<CF>,
    // Importantly, `Cache`s must be dropped after `DB`, so we place them as the last field
    // (fields in a struct are dropped in the declaration order).
    _caches: RocksDBCaches,
}

impl<CF: NamedColumnFamily> RocksDB<CF> {
    fn column_family(&self, cf: CF) -> &ColumnFamily {
        self.db
            .cf_handle(cf.name())
            .unwrap_or_else(|| panic!("Column family `{}` doesn't exist", cf.name()))
    }

    pub fn get_cf(&self, cf: CF, key: &[u8]) -> Result<Option<Vec<u8>>, rocksdb::Error> {
        let cf = self.column_family(cf);
        self.db.get_cf(cf, key)
    }

    pub fn new_write_batch(&self) -> WriteBatch<'_, CF> {
        WriteBatch {
            inner: rocksdb::WriteBatch::default(),
            db: self,
        }
    }

    pub fn write<'a>(&'a self, batch: WriteBatch<'a, CF>) -> Result<(), rocksdb::Error> {
        let raw_batch = batch.inner;
        let write_metrics = WriteMetrics {
            batch_size: raw_batch.size_in_bytes() as u64,
        };

        if self.sync_writes {
            let mut options = WriteOptions::new();
            options.set_sync(true);
            self.db.write_opt(raw_batch, &options)?;
        } else {
            self.db.write(raw_batch)?;
        }

        write_metrics.report(CF::DB_NAME);
        Ok(())
    }

    pub fn estimated_number_of_entries(&self, cf: CF) -> u64 {
        const ERROR_MSG: &str = "failed to get estimated number of entries";

        let cf = self.db.cf_handle(cf.name()).unwrap();
        self.db
            .property_int_value_cf(cf, properties::ESTIMATE_NUM_KEYS)
            .expect(ERROR_MSG)
            .unwrap_or(0)
    }
}

struct RocksDBCaches {
    /// LRU block cache shared among all column families.
    shared: Option<Cache>,
}

impl<CF: NamedColumnFamily> RocksDB<CF> {
    const SIZE_REPORT_INTERVAL: Duration = Duration::from_secs(1);

    pub fn new<P: AsRef<Path>>(path: P, tune_options: bool) -> Self {
        Self::with_cache(path, tune_options, None)
    }

    pub fn with_cache<P: AsRef<Path>>(
        path: P,
        tune_options: bool,
        block_cache_capacity: Option<usize>,
    ) -> Self {
        describe_metrics();

        let caches = RocksDBCaches::new(block_cache_capacity);
        let options = Self::rocksdb_options(tune_options, None);
        let existing_cfs = DB::list_cf(&options, path.as_ref()).unwrap_or_else(|err| {
            olaos_logs::warn!(
                "Failed getting column families for RocksDB `{}` at `{}`, assuming CFs are empty; {err}",
                CF::DB_NAME,
                path.as_ref().display()
            );
            vec![]
        });

        let cf_names: HashSet<_> = CF::ALL.iter().map(|cf| cf.name()).collect();
        let obsolete_cfs: Vec<_> = existing_cfs
            .iter()
            .filter_map(|cf_name| {
                let cf_name = cf_name.as_str();
                // The default CF is created on RocksDB instantiation in any case; it doesn't need
                // to be explicitly opened.
                let is_obsolete =
                    cf_name != rocksdb::DEFAULT_COLUMN_FAMILY_NAME && !cf_names.contains(cf_name);
                is_obsolete.then_some(cf_name)
            })
            .collect();
        if !obsolete_cfs.is_empty() {
            olaos_logs::warn!(
                "RocksDB `{}` at `{}` contains extra column families {obsolete_cfs:?} that are not used \
                 in code",
                CF::DB_NAME,
                path.as_ref().display()
            );
        }

        // Open obsolete CFs as well; RocksDB initialization will panic otherwise.
        let cfs = cf_names.into_iter().chain(obsolete_cfs).map(|cf_name| {
            let mut block_based_options = BlockBasedOptions::default();
            if tune_options {
                block_based_options.set_bloom_filter(10.0, false);
            }
            if let Some(cache) = &caches.shared {
                block_based_options.set_block_cache(cache);
            }
            let cf_options = Self::rocksdb_options(tune_options, Some(block_based_options));
            ColumnFamilyDescriptor::new(cf_name, cf_options)
        });
        let db = DB::open_cf_descriptors(&options, path, cfs).expect("failed to init rocksdb");

        Self {
            db,
            sync_writes: false,
            sizes_reported_at: Mutex::new(None),
            _registry_entry: RegistryEntry::new(),
            _cf: PhantomData,
            _caches: caches,
        }
    }

    fn rocksdb_options(
        tune_options: bool,
        block_based_options: Option<BlockBasedOptions>,
    ) -> Options {
        let mut options = Options::default();
        options.create_missing_column_families(true);
        options.create_if_missing(true);
        if tune_options {
            options.increase_parallelism(num_cpus::get() as i32);
        }
        if let Some(block_based_options) = block_based_options {
            options.set_block_based_table_factory(&block_based_options);
        }
        options
    }
}

impl fmt::Debug for RocksDBCaches {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RocksDBCaches")
            .finish_non_exhaustive()
    }
}

impl RocksDBCaches {
    fn new(capacity: Option<usize>) -> Self {
        let shared = capacity.map(Cache::new_lru_cache);
        Self { shared }
    }
}

#[derive(Debug)]
struct RegistryEntry;

impl RegistryEntry {
    fn new() -> Self {
        let (lock, cvar) = &ROCKSDB_INSTANCE_COUNTER;
        let mut num_instances = lock.lock().unwrap();
        *num_instances += 1;
        cvar.notify_all();
        Self
    }
}

impl Drop for RegistryEntry {
    fn drop(&mut self) {
        let (lock, cvar) = &ROCKSDB_INSTANCE_COUNTER;
        let mut num_instances = lock.lock().unwrap();
        *num_instances -= 1;
        cvar.notify_all();
    }
}
