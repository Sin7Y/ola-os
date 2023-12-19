use ola_config::sequencer::SequencerConfig;
use ola_types::{tx::tx_execution_info::{DeduplicatedWritesMetrics, ExecutionMetrics}, circuit::GEOMETRY_CONFIG};

use crate::sequencer::{
    seal_criteria::{SealCriterion, SealResolution},
    SealData,
};

#[derive(Debug, Default)]
pub struct RepeatedWritesCriterion;
#[derive(Debug, Default)]
pub struct InitialWritesCriterion;

trait MetricExtractor {
    const PROM_METRIC_CRITERION_NAME: &'static str;
    fn limit_per_block() -> usize;
    fn extract(metric: &ExecutionMetrics, writes: &DeduplicatedWritesMetrics) -> usize;
}

impl<T> SealCriterion for T
where
    T: MetricExtractor + std::fmt::Debug + Send + Sync + 'static,
{
    fn should_seal(
        &self,
        config: &SequencerConfig,
        _block_open_timestamp_ms: u128,
        _tx_count: usize,
        block_data: &SealData,
        tx_data: &SealData,
    ) -> SealResolution {
        let reject_bound =
            (T::limit_per_block() as f64 * config.reject_tx_at_geometry_percentage).round();
        let close_bound =
            (T::limit_per_block() as f64 * config.close_block_at_geometry_percentage).round();

        if T::extract(&tx_data.execution_metrics, &tx_data.writes_metrics) > reject_bound as usize {
            SealResolution::Unexecutable("ZK proof cannot be generated for a transaction".into())
        } else if T::extract(&block_data.execution_metrics, &block_data.writes_metrics)
            >= T::limit_per_block()
        {
            SealResolution::ExcludeAndSeal
        } else if T::extract(&block_data.execution_metrics, &block_data.writes_metrics)
            > close_bound as usize
        {
            SealResolution::IncludeAndSeal
        } else {
            SealResolution::NoSeal
        }
    }

    fn prom_criterion_name(&self) -> &'static str {
        T::PROM_METRIC_CRITERION_NAME
    }
}

impl MetricExtractor for RepeatedWritesCriterion {
    const PROM_METRIC_CRITERION_NAME: &'static str = "repeated_storage_writes";

    fn limit_per_block() -> usize {
        GEOMETRY_CONFIG.limit_for_repeated_writes_pubdata_hasher as usize
    }

    fn extract(_metrics: &ExecutionMetrics, writes: &DeduplicatedWritesMetrics) -> usize {
        writes.repeated_storage_writes
    }
}

impl MetricExtractor for InitialWritesCriterion {
    const PROM_METRIC_CRITERION_NAME: &'static str = "initial_storage_writes";

    fn limit_per_block() -> usize {
        GEOMETRY_CONFIG.limit_for_initial_writes_pubdata_hasher as usize
    }

    fn extract(_metrics: &ExecutionMetrics, writes: &DeduplicatedWritesMetrics) -> usize {
        writes.initial_storage_writes
    }
}
