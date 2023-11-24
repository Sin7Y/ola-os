use std::time::Duration;
use ola_config::{
    chain::OperationsManagerConfig,
    database::DBConfig,
};

pub mod helpers;

/// Configuration of [`MetadataCalculator`].
#[derive(Debug)]
pub struct MetadataCalculatorConfig<'a> {
    /// Filesystem path to the RocksDB instance that stores the tree.
    pub db_path: &'a str,
    /// Interval between polling Postgres for updates if no progress was made by the tree.
    pub delay_interval: Duration,
    /// Maximum number of L1 batches to get from Postgres on a single update iteration.
    pub max_l1_batches_per_iter: usize,
    /// Chunk size for multi-get operations. Can speed up loading data for the Merkle tree on some environments,
    /// but the effects vary wildly depending on the setup (e.g., the filesystem used).
    pub multi_get_chunk_size: usize,
    /// Capacity of RocksDB block cache in bytes. Reasonable values range from ~100 MB to several GB.
    pub block_cache_capacity: usize,
}

impl<'a> MetadataCalculatorConfig<'a> {
    pub(crate) fn for_main_node(
        db_config: &'a DBConfig,
        operation_config: &'a OperationsManagerConfig,
    ) -> Self {
        Self {
            db_path: &db_config.merkle_tree.path,
            delay_interval: operation_config.delay_interval(),
            max_l1_batches_per_iter: db_config.merkle_tree.max_l1_batches_per_iter,
            multi_get_chunk_size: db_config.merkle_tree.multi_get_chunk_size,
            block_cache_capacity: db_config.merkle_tree.block_cache_size(),
        }
    }
}

// #[derive(Debug)]
// pub struct MetadataCalculator {
//     updater: TreeUpdater,
//     delayer: Delayer,
//     health_updater: HealthUpdater,
// }