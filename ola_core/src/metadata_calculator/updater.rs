use tokio::sync::watch;
use super::{
    helpers::{AsyncTree, Delayer}, MetadataCalculator, MetadataCalculatorConfig,
};
use ola_dal::connection::ConnectionPool;
use olaos_health_check::HealthUpdater;

#[derive(Debug)]
pub(super) struct TreeUpdater {
    tree: AsyncTree,
    max_l1_batches_per_iter: usize,
}

impl TreeUpdater {
    pub async fn new(
        config: &MetadataCalculatorConfig<'_>
    ) -> Self {
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
            max_l1_batches_per_iter: config.max_l1_batches_per_iter
        }
    }

    /// The processing loop for this updater.
    pub async fn loop_updating_tree(
        mut self,
        delayer: Delayer,
        pool: &ConnectionPool,
        prover_pool: &ConnectionPool,
        mut stop_receiver: watch::Receiver<bool>,
        health_updater: HealthUpdater,
    ) {

    }
}