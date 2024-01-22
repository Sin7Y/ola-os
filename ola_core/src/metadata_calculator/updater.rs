use super::{
    helpers::{get_logs_for_l1_batch, AsyncTree, Delayer, TreeHealthCheckDetails},
    MetadataCalculator, MetadataCalculatorConfig,
};
use futures::{future::ready as future_ready, stream::StreamExt, FutureExt};

use ola_dal::{connection::ConnectionPool, StorageProcessor};
use ola_types::{block::WitnessBlockWithLogs, L1BatchNumber};
use olaos_health_check::HealthUpdater;
use olavm_core::types::merkle_tree::tree_key_to_h256;
use tokio::sync::watch;

#[derive(Debug)]
pub(super) struct TreeUpdater {
    tree: AsyncTree,
    max_l1_batches_per_iter: usize,
}

impl TreeUpdater {
    pub async fn new(config: &MetadataCalculatorConfig<'_>) -> Self {
        assert!(
            config.max_l1_batches_per_iter > 0,
            "Maximum L1 batches per iteration is misconfigured to be 0; please update it to positive value"
        );

        let db_path = config.db_path.into();
        let tree = AsyncTree::new(
            db_path,
            config.multi_get_chunk_size,
            config.block_cache_capacity,
        )
        .await;
        Self {
            tree,
            max_l1_batches_per_iter: config.max_l1_batches_per_iter,
        }
    }

    #[tracing::instrument(skip(self, storage, blocks))]
    async fn process_multiple_blocks(
        &mut self,
        storage: &mut StorageProcessor<'_>,
        blocks: Vec<WitnessBlockWithLogs>,
    ) {
        let storage_logs = blocks.iter().map(|block| block.storage_logs.as_slice());

        let mut previous_root_hash = self.tree.root_hash();
        let metadata = self.tree.process_blocks(storage_logs).await;

        for (metadata_at_block, block) in metadata.into_iter().zip(blocks) {
            let next_root_hash = tree_key_to_h256(&metadata_at_block.root_hash);
            let metadata =
                MetadataCalculator::build_block_metadata(metadata_at_block, &block.header);

            storage
                .blocks_dal()
                .save_l1_batch_metadata(block.header.number, metadata, previous_root_hash)
                .await;

            previous_root_hash = next_root_hash;
        }

        self.tree.save().await;
    }

    pub async fn step(
        &mut self,
        mut storage: StorageProcessor<'_>,
        next_block_to_seal: &mut L1BatchNumber,
    ) {
        let mut new_blocks = vec![];
        let last_sealed_l1_batch = storage.blocks_dal().get_sealed_l1_batch_number().await;
        let mut index = 0;
        for block_number in next_block_to_seal.0..=last_sealed_l1_batch.0 {
            if index >= self.max_l1_batches_per_iter {
                break;
            }

            let logs = get_logs_for_l1_batch(&mut storage, L1BatchNumber(block_number))
                .await
                .unwrap();
            new_blocks.push(logs);
            index += 1;
        }

        for block in new_blocks {
            *next_block_to_seal = block.header.number + 1;
            self.process_multiple_blocks(&mut storage, vec![block]).await;
        }
    }

    /// The processing loop for this updater.
    pub async fn loop_updating_tree(
        mut self,
        delayer: Delayer,
        pool: &ConnectionPool,
        _prover_pool: &ConnectionPool,
        mut stop_receiver: watch::Receiver<bool>,
        health_updater: HealthUpdater,
    ) {
        let mut storage = pool.access_storage_tagged("metadata_calculator").await;

        // Ensure genesis creation
        let tree = &mut self.tree;
        if tree.is_empty() {
            let Some(logs) =
                crate::metadata_calculator::get_logs_for_l1_batch(&mut storage, L1BatchNumber(0))
                    .await
            else {
                panic!("Missing storage logs for the genesis block");
            };
            tree.process_block(&logs.storage_logs).await;
            tree.save().await;
        }
        let mut next_l1_batch_to_seal = L1BatchNumber(tree.block_number());

        let current_db_batch = storage.blocks_dal().get_sealed_l1_batch_number().await + 1;
        let last_l1_batch_with_metadata = storage
            .blocks_dal()
            .get_last_l1_batch_number_with_metadata()
            .await
            + 1;
        drop(storage);

        olaos_logs::info!(
            "Initialized metadata calculator with merkle tree implementation. \
             Current RocksDB block: {}. Current Postgres block: {}",
            next_l1_batch_to_seal,
            current_db_batch
        );
        let backup_lag = last_l1_batch_with_metadata
            .0
            .saturating_sub(next_l1_batch_to_seal.0);
        metrics::gauge!("server.metadata_calculator.backup_lag", backup_lag as f64);

        let health = TreeHealthCheckDetails {
            next_l1_batch_to_seal,
        };
        health_updater.update(health.into());

        loop {
            if *stop_receiver.borrow_and_update() {
                olaos_logs::info!("Stop signal received, metadata_calculator is shutting down");
                break;
            }
            let storage = pool.access_storage_tagged("metadata_calculator").await;

            let next_block_snapshot = *next_l1_batch_to_seal;
            self.step(storage, &mut next_l1_batch_to_seal).await;
            let delay = if next_block_snapshot == *next_l1_batch_to_seal {
                // We didn't make any progress.
                delayer.wait(&self.tree).left_future()
            } else {
                let health = TreeHealthCheckDetails {
                    next_l1_batch_to_seal,
                };
                health_updater.update(health.into());

                olaos_logs::trace!(
                    "Metadata calculator (next L1 batch: #{next_l1_batch_to_seal}) made progress from #{next_block_snapshot}"
                );
                future_ready(()).right_future()
            };

            // The delays we're operating with are reasonably small, but selecting between the delay
            // and the stop receiver still allows to be more responsive during shutdown.
            tokio::select! {
                _ = stop_receiver.changed() => {
                    olaos_logs::info!("Stop signal received, metadata_calculator is shutting down");
                    break;
                }
                () = delay => { /* The delay has passed */ }
            }
        }
        drop(health_updater); // Explicitly mark where the updater should be dropped
    }
}
