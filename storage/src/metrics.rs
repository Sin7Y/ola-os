use std::sync::Once;

use metrics::Unit;

pub(crate) fn describe_metrics() {
    static INITIALIZER: Once = Once::new();

    INITIALIZER.call_once(|| {
        WriteMetrics::describe();
        RocksDBSizeStats::describe();
    });
}

#[must_use = "write metrics should be `report()`ed"]
#[derive(Debug)]
pub(crate) struct WriteMetrics {
    pub batch_size: u64,
}

impl WriteMetrics {
    const BATCH_SIZE: &'static str = "rocksdb.write.batch_size";

    fn describe() {
        metrics::describe_histogram!(
            Self::BATCH_SIZE,
            Unit::Bytes,
            "Size of a serialized `WriteBatch` written to a RocksDB instance"
        );
    }

    pub fn report(self, db_name: &'static str) {
        metrics::histogram!(Self::BATCH_SIZE, self.batch_size as f64, "db" => db_name);
    }
}

#[must_use = "stats should be `report()`ed"]
#[derive(Debug)]
pub(crate) struct RocksDBSizeStats {
    /// Estimated size of all live data in the DB in bytes.
    pub estimated_live_data_size: u64,
    /// Total size of all SST files in bytes.
    pub total_sst_file_size: u64,
    /// Total size of all memtables in bytes.
    pub total_mem_table_size: u64,
    /// Total size of block cache.
    pub block_cache_size: u64,
    /// Total size of index and Bloom filter blocks.
    pub index_and_filters_size: u64,
}

impl RocksDBSizeStats {
    const ESTIMATED_LIVE_DATA_SIZE: &'static str = "rocksdb.live_data_size";
    const TOTAL_SST_FILE_SIZE: &'static str = "rocksdb.total_sst_size";
    const TOTAL_MEM_TABLE_SIZE: &'static str = "rocksdb.total_mem_table_size";
    const BLOCK_CACHE_SIZE: &'static str = "rocksdb.block_cache_size";
    const INDEX_AND_FILTERS_SIZE: &'static str = "rocksdb.index_and_filters_size";

    fn describe() {
        metrics::describe_gauge!(
            Self::ESTIMATED_LIVE_DATA_SIZE,
            Unit::Bytes,
            "Estimated size of all live data in the column family of a RocksDB instance"
        );
        metrics::describe_gauge!(
            Self::TOTAL_SST_FILE_SIZE,
            Unit::Bytes,
            "Total size of all SST files in the column family of a RocksDB instance"
        );
        metrics::describe_gauge!(
            Self::TOTAL_MEM_TABLE_SIZE,
            Unit::Bytes,
            "Total size of all mem tables in the column family of a RocksDB instance"
        );
        metrics::describe_gauge!(
            Self::BLOCK_CACHE_SIZE,
            Unit::Bytes,
            "Total size of block cache in the column family of a RocksDB instance"
        );
        metrics::describe_gauge!(
            Self::INDEX_AND_FILTERS_SIZE,
            Unit::Bytes,
            "Total size of index and Bloom filters in the column family of a RocksDB instance"
        );
    }
}
