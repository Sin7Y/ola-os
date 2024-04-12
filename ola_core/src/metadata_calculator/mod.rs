use std::{sync::Arc, time::Duration};

use anyhow::Ok;
use ola_config::{
    chain::OperationsManagerConfig,
    database::{MerkleTreeConfig, MerkleTreeMode},
};
use ola_dal::connection::ConnectionPool;
use ola_types::merkle_tree::{tree_key_to_h256, TreeMetadata};
use ola_types::{
    block::{L1BatchHeader, WitnessBlockWithLogs},
    commitment::{L1BatchCommitment, L1BatchMetadata},
    H256,
};
use olaos_health_check::{HealthUpdater, ReactiveHealthCheck};
use olaos_merkle_tree::tree::AccountTree;
use olaos_object_store::ObjectStore;
use tokio::sync::watch;

use tempfile::TempDir;

pub(crate) use self::helpers::get_logs_for_l1_batch;
use self::{
    helpers::{create_db, AsyncTree, Delayer},
    updater::TreeUpdater,
};

mod helpers;
mod updater;

/// Configuration of [`MetadataCalculator`].
#[derive(Debug)]
pub struct MetadataCalculatorConfig {
    /// Filesystem path to the RocksDB instance that stores the tree.
    pub db_path: String,
    /// Configuration of the Merkle tree mode.
    pub mode: MerkleTreeMode,
    /// Interval between polling Postgres for updates if no progress was made by the tree.
    pub delay_interval: Duration,
    /// Maximum number of L1 batches to get from Postgres on a single update iteration.
    pub max_l1_batches_per_iter: usize,
    /// Chunk size for multi-get operations. Can speed up loading data for the Merkle tree on some environments,
    /// but the effects vary wildly depending on the setup (e.g., the filesystem used).
    pub multi_get_chunk_size: usize,
    /// Capacity of RocksDB block cache in bytes. Reasonable values range from ~100 MiB to several GB.
    pub block_cache_capacity: usize,
    /// Capacity of RocksDB memtables. Can be set to a reasonably large value (order of 512 MiB)
    /// to mitigate write stalls.
    pub memtable_capacity: usize,
    /// Timeout to wait for the Merkle tree database to run compaction on stalled writes.
    pub stalled_writes_timeout: Duration,
}

impl MetadataCalculatorConfig {
    pub(crate) fn for_main_node(
        merkle_tree_config: &MerkleTreeConfig,
        operation_config: &OperationsManagerConfig,
    ) -> Self {
        Self {
            db_path: merkle_tree_config.path.clone(),
            mode: merkle_tree_config.mode,
            delay_interval: operation_config.delay_interval(),
            max_l1_batches_per_iter: merkle_tree_config.max_l1_batches_per_iter,
            multi_get_chunk_size: merkle_tree_config.multi_get_chunk_size,
            block_cache_capacity: merkle_tree_config.block_cache_size(),
            memtable_capacity: merkle_tree_config.memtable_capacity(),
            stalled_writes_timeout: merkle_tree_config.stalled_writes_timeout(),
        }
    }
}

#[derive(Debug)]
pub struct MetadataCalculator {
    tree: AsyncTree,
    object_store: Option<Arc<dyn ObjectStore>>,
    delayer: Delayer,
    health_updater: HealthUpdater,
    max_l1_batches_per_iter: usize,
}

impl MetadataCalculator {
    /// Creates a calculator with the specified `config`.
    pub async fn new(
        config: MetadataCalculatorConfig,
        object_store: Option<Arc<dyn ObjectStore>>,
    ) -> Self {
        assert!(
            config.max_l1_batches_per_iter > 0,
            "Maximum L1 batches per iteration is misconfigured to be 0; please update it to positive value"
        );

        let db = create_db(
            config.db_path.clone().into(),
            config.block_cache_capacity,
            config.memtable_capacity,
            config.stalled_writes_timeout,
            config.multi_get_chunk_size,
        )
        .await;
        let tree = AsyncTree::new(db);

        let (_, health_updater) = ReactiveHealthCheck::new("tree");
        Self {
            tree,
            object_store,
            delayer: Delayer::new(config.delay_interval),
            health_updater,
            max_l1_batches_per_iter: config.max_l1_batches_per_iter,
        }
    }

    /// Returns a health check for this calculator.
    pub fn tree_health_check(&self) -> ReactiveHealthCheck {
        self.health_updater.subscribe()
    }

    pub async fn run(
        self,
        pool: ConnectionPool,
        stop_receiver: watch::Receiver<bool>,
    ) -> anyhow::Result<()> {
        let updater = TreeUpdater::new(self.tree, self.max_l1_batches_per_iter, self.object_store);
        updater
            .loop_updating_tree(self.delayer, &pool, stop_receiver, self.health_updater)
            .await;
        Ok(())
    }

    // TODO: gas
    // This is used to improve L1 gas estimation for the commit operation. The estimations are computed
    // in the State Keeper, where storage writes aren't yet deduplicated, whereas L1 batch metadata
    // contains deduplicated storage writes.
    // async fn reestimate_l1_batch_commit_gas(
    //     storage: &mut StorageProcessor<'_>,
    //     header: &L1BatchHeader,
    //     metadata: &L1BatchMetadata,
    // ) {
    //     let unsorted_factory_deps = storage
    //         .blocks_dal()
    //         .get_l1_batch_factory_deps(header.number)
    //         .await
    //         .unwrap();
    //     let commit_gas_cost =
    //         commit_gas_count_for_l1_batch(header, &unsorted_factory_deps, metadata);
    //     storage
    //         .blocks_dal()
    //         .update_predicted_l1_batch_commit_gas(header.number, commit_gas_cost)
    //         .await
    //         .unwrap();
    // }

    fn build_l1_batch_metadata(
        tree_metadata: TreeMetadata,
        header: &L1BatchHeader,
    ) -> L1BatchMetadata {
        let merkle_root_hash = tree_key_to_h256(&tree_metadata.root_hash);
        let commitment = L1BatchCommitment::new(
            tree_metadata.rollup_last_leaf_index,
            merkle_root_hash,
            tree_metadata.initial_writes,
            tree_metadata.repeated_writes,
            header.base_system_contracts_hashes.entrypoint,
            header.base_system_contracts_hashes.default_aa,
        );
        let commitment_hash = commitment.hash();
        olaos_logs::trace!("L1 batch commitment: {commitment:?}");

        let metadata = L1BatchMetadata {
            root_hash: merkle_root_hash,
            rollup_last_leaf_index: tree_metadata.rollup_last_leaf_index,
            merkle_root_hash,
            initial_writes_compressed: commitment.initial_writes_compressed().to_vec(),
            repeated_writes_compressed: commitment.repeated_writes_compressed().to_vec(),
            commitment: commitment_hash.commitment,
            block_meta_params: commitment.meta_parameters(),
            aux_data_hash: commitment_hash.aux_output,
            meta_parameters_hash: commitment_hash.meta_parameters,
            pass_through_data_hash: commitment_hash.pass_through_data,
            // TODO: use real l2_l1_messages_compressed,l2_l1_merkle_root,state_diffs_compressed,events_queue_commitment
            l2_l1_messages_compressed: vec![],
            l2_l1_merkle_root: H256::default(),
            state_diffs_compressed: vec![],
            events_queue_commitment: None,
            // state_diffs_compressed: commitment.state_diffs_compressed().to_vec(),
            // events_queue_commitment,
            // bootloader_initial_content_commitment,
        };

        olaos_logs::trace!("L1 batch metadata: {metadata:?}");
        metadata
    }

    pub fn process_genesis_batch(l1_batch: WitnessBlockWithLogs) -> TreeMetadata {
        let temp_dir = TempDir::new().expect("failed get temporary directory for RocksDB");
        let mut tree = AccountTree::new(temp_dir.as_ref());
        tree.process_block(&l1_batch.storage_logs)
    }
}
