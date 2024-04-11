//! Tree updater trait and its implementations.

use futures::{future::join as future_join, future::ready as future_ready, FutureExt};
use ola_dal::{connection::ConnectionPool, StorageProcessor};
use ola_types::{
    block::{L1BatchHeader, WitnessBlockWithLogs},
    merkle_tree::TreeMetadata,
    writes::InitialStorageWrite,
    L1BatchNumber, U256,
};
use olaos_health_check::HealthUpdater;
use olaos_object_store::ObjectStore;
use std::{ops, sync::Arc};
use tokio::sync::watch;

use super::{
    helpers::{get_logs_for_l1_batch, AsyncTree, Delayer, TreeHealthCheckDetails},
    MetadataCalculator,
};

#[derive(Debug)]
pub(super) struct TreeUpdater {
    tree: AsyncTree,
    max_l1_batches_per_iter: usize,
    object_store: Option<Arc<dyn ObjectStore>>,
}

impl TreeUpdater {
    pub fn new(
        tree: AsyncTree,
        max_l1_batches_per_iter: usize,
        object_store: Option<Arc<dyn ObjectStore>>,
    ) -> Self {
        Self {
            tree,
            max_l1_batches_per_iter,
            object_store,
        }
    }

    async fn process_l1_batch(
        &mut self,
        l1_batch: WitnessBlockWithLogs,
    ) -> (L1BatchHeader, TreeMetadata, Option<String>) {
        let mut metadata = self.tree.process_l1_batch(l1_batch.storage_logs).await;

        let witness_input = metadata.witness.take();
        let l1_batch_number = l1_batch.header.number;
        let object_key = if let Some(object_store) = &self.object_store {
            let witness_input =
                witness_input.expect("No witness input provided by tree; this is a bug");
            let object_key = object_store
                .put(l1_batch_number, &witness_input)
                .await
                .unwrap();

            olaos_logs::info!(
                "Saved witnesses for L1 batch #{l1_batch_number} to object storage at `{object_key}`"
            );
            Some(object_key)
        } else {
            None
        };

        (l1_batch.header, metadata, object_key)
    }

    async fn process_multiple_batches(
        &mut self,
        storage: &mut StorageProcessor<'_>,
        l1_batch_numbers: ops::RangeInclusive<u32>,
    ) -> L1BatchNumber {
        olaos_logs::info!("Processing L1 batches #{l1_batch_numbers:?}");
        let first_l1_batch_number = *l1_batch_numbers.start();
        let last_l1_batch_number = L1BatchNumber(*l1_batch_numbers.end());
        let mut l1_batch_data =
            get_logs_for_l1_batch(storage, L1BatchNumber(first_l1_batch_number)).await;

        let mut previous_root_hash = self.tree.root_hash();
        let mut updated_headers = vec![];
        for l1_batch_number in l1_batch_numbers {
            let l1_batch_number = L1BatchNumber(l1_batch_number);
            let Some(current_l1_batch_data) = l1_batch_data else {
                return l1_batch_number;
            };

            let process_l1_batch_task = self.process_l1_batch(current_l1_batch_data);
            let load_next_l1_batch_task = async {
                if l1_batch_number < last_l1_batch_number {
                    get_logs_for_l1_batch(storage, l1_batch_number + 1).await
                } else {
                    None // Don't need to load the next L1 batch after the last one we're processing.
                }
            };
            let ((header, metadata, object_key), next_l1_batch_data) =
                future_join(process_l1_batch_task, load_next_l1_batch_task).await;

            Self::check_initial_writes_consistency(
                storage,
                header.number,
                &metadata.initial_writes,
            )
            .await;

            let metadata = MetadataCalculator::build_l1_batch_metadata(metadata, &header);

            // TODO: gas
            // MetadataCalculator::reestimate_l1_batch_commit_gas(storage, &header, &metadata).await;
            storage
                .blocks_dal()
                .save_l1_batch_metadata(l1_batch_number, &metadata, previous_root_hash)
                .await
                .unwrap();
            // ^ Note that `save_l1_batch_metadata()` will not blindly overwrite changes if L1 batch
            // metadata already exists; instead, it'll check that the old and new metadata match.
            // That is, if we run multiple tree instances, we'll get metadata correspondence
            // right away without having to implement dedicated code.

            if let Some(object_key) = &object_key {
                storage
                    .basic_witness_input_producer_dal()
                    .create_basic_witness_input_producer_job(l1_batch_number)
                    .await
                    .expect("failed to create basic_witness_input_producer job");
                storage
                    .proof_generation_dal()
                    .insert_proof_generation_details(l1_batch_number, object_key)
                    .await;
            }
            olaos_logs::info!("Updated metadata for L1 batch #{l1_batch_number} in Postgres");

            previous_root_hash = metadata.merkle_root_hash;
            updated_headers.push(header);
            l1_batch_data = next_l1_batch_data;
        }

        self.tree.save().await;

        last_l1_batch_number + 1
    }

    async fn step(
        &mut self,
        mut storage: StorageProcessor<'_>,
        next_l1_batch_to_seal: &mut L1BatchNumber,
    ) {
        let last_sealed_l1_batch = storage.blocks_dal().get_sealed_l1_batch_number().await;
        let last_requested_l1_batch =
            next_l1_batch_to_seal.0 + self.max_l1_batches_per_iter as u32 - 1;
        let last_requested_l1_batch = last_requested_l1_batch.min(last_sealed_l1_batch.0);
        let l1_batch_numbers = next_l1_batch_to_seal.0..=last_requested_l1_batch;
        if l1_batch_numbers.is_empty() {
            olaos_logs::trace!(
                "No L1 batches to seal: batch numbers range to be loaded {l1_batch_numbers:?} is empty"
            );
        } else {
            olaos_logs::info!("Updating Merkle tree with L1 batches #{l1_batch_numbers:?}");
            *next_l1_batch_to_seal = self
                .process_multiple_batches(&mut storage, l1_batch_numbers)
                .await;
        }
    }

    /// The processing loop for this updater.
    pub async fn loop_updating_tree(
        mut self,
        delayer: Delayer,
        pool: &ConnectionPool,
        mut stop_receiver: watch::Receiver<bool>,
        health_updater: HealthUpdater,
    ) {
        let mut storage = pool.access_storage_tagged("metadata_calculator").await;

        // Ensure genesis creation
        let tree = &mut self.tree;
        if tree.is_empty() {
            let Some(logs) = get_logs_for_l1_batch(&mut storage, L1BatchNumber(0)).await else {
                panic!("Missing storage logs for the genesis block");
            };
            let _ = tree.process_l1_batch(logs.storage_logs).await;
            tree.save().await;
        }
        let mut next_l1_batch_to_seal = tree.next_l1_batch_number();

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

                olaos_logs::info!(
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

    async fn check_initial_writes_consistency(
        connection: &mut StorageProcessor<'_>,
        l1_batch_number: L1BatchNumber,
        tree_initial_writes: &[InitialStorageWrite],
    ) {
        let pg_initial_writes = connection
            .storage_logs_dedup_dal()
            .initial_writes_for_batch(l1_batch_number)
            .await;

        let pg_initial_writes: Vec<_> = pg_initial_writes
            .into_iter()
            .map(|(key, index)| {
                let key = U256::from_little_endian(key.as_bytes());
                (key, index.map_or(0, |n| n as u64))
            })
            .collect();

        let tree_initial_writes: Vec<_> = tree_initial_writes
            .iter()
            .map(|write| (write.key, write.index))
            .collect();
        assert_eq!(
            pg_initial_writes, tree_initial_writes,
            "Leaf indices are not consistent for L1 batch {l1_batch_number}"
        );
    }
}
