use ola_config::{chain::OperationsManagerConfig, database::DBConfig};
use ola_dal::connection::ConnectionPool;
use ola_types::{
    block::L1BatchHeader,
    commitment::{L1BatchCommitment, L1BatchMetadata},
    log::StorageLog,
    storage::writes::{InitialStorageWrite, RepeatedStorageWrite},
};
use olaos_health_check::{HealthUpdater, ReactiveHealthCheck};
use olavm_core::types::merkle_tree::{tree_key_to_h256, tree_key_to_u256, TreeMetadata};
use std::time::Duration;
use tokio::sync::watch;

mod helpers;
mod updater;

pub(crate) use self::helpers::get_logs_for_l1_batch;
pub use self::helpers::AsyncTree;

use self::helpers::Delayer;
use self::updater::TreeUpdater;

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

#[derive(Debug)]
pub struct MetadataCalculator {
    updater: TreeUpdater,
    delayer: Delayer,
    health_updater: HealthUpdater,
}

impl MetadataCalculator {
    /// Creates a calculator with the specified `config`.
    pub async fn new(config: &MetadataCalculatorConfig<'_>) -> Self {
        let updater = TreeUpdater::new(config).await;
        let (_, health_updater) = ReactiveHealthCheck::new("tree");
        Self {
            updater,
            delayer: Delayer::new(config.delay_interval),
            health_updater,
        }
    }

    /// Returns a health check for this calculator.
    pub fn tree_health_check(&self) -> ReactiveHealthCheck {
        self.health_updater.subscribe()
    }

    pub async fn run(
        self,
        pool: ConnectionPool,
        prover_pool: ConnectionPool,
        stop_receiver: watch::Receiver<bool>,
    ) {
        let update_task = self.updater.loop_updating_tree(
            self.delayer,
            &pool,
            &prover_pool,
            stop_receiver,
            self.health_updater,
        );
        update_task.await;
    }

    fn build_block_metadata(
        tree_metadata_at_block: TreeMetadata,
        l1_batch_header: &L1BatchHeader,
    ) -> L1BatchMetadata {
        let merkle_root_hash = tree_key_to_h256(&tree_metadata_at_block.root_hash);
        let initial_writes = tree_metadata_at_block
            .initial_writes
            .iter()
            .map(|iw| InitialStorageWrite {
                key: tree_key_to_u256(&iw.key),
                value: tree_key_to_h256(&iw.value),
            })
            .collect::<Vec<InitialStorageWrite>>();
        let repeated_writes = tree_metadata_at_block
            .repeated_writes
            .iter()
            .map(|iw| RepeatedStorageWrite {
                index: 0,
                value: tree_key_to_h256(&iw.value),
            })
            .collect::<Vec<RepeatedStorageWrite>>();
        let block_commitment = L1BatchCommitment::new(
            tree_metadata_at_block.rollup_last_leaf_index,
            merkle_root_hash,
            initial_writes,
            repeated_writes,
            l1_batch_header.base_system_contracts_hashes.entrypoint,
            l1_batch_header.base_system_contracts_hashes.default_aa,
        );
        let block_commitment_hash = block_commitment.hash();
        olaos_logs::trace!("L1 batch commitment {:?}", &block_commitment);

        let metadata = L1BatchMetadata {
            root_hash: merkle_root_hash,
            rollup_last_leaf_index: tree_metadata_at_block.rollup_last_leaf_index,
            merkle_root_hash,
            initial_writes_compressed: block_commitment.initial_writes_compressed().to_vec(),
            repeated_writes_compressed: block_commitment.repeated_writes_compressed().to_vec(),
            commitment: block_commitment_hash.commitment,
            block_meta_params: block_commitment.meta_parameters(),
            aux_data_hash: block_commitment_hash.aux_output,
            meta_parameters_hash: block_commitment_hash.meta_parameters,
            pass_through_data_hash: block_commitment_hash.pass_through_data,
        };

        olaos_logs::trace!("Block metadata {:?}", metadata);
        metadata
    }
}
