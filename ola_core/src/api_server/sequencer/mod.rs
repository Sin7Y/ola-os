use ola_types::{
    fee::TransactionExecutionMetrics,
    tx::tx_execution_info::{DeduplicatedWritesMetrics, ExecutionMetrics},
    Transaction,
};

pub mod extractors;
pub mod seal_criteria;

#[derive(Debug, Default)]
pub struct SealData {
    pub(super) execution_metrics: ExecutionMetrics,
    pub(super) cumulative_size: usize,
    pub(super) writes_metrics: DeduplicatedWritesMetrics,
}

impl SealData {
    pub(crate) fn for_transaction(
        transaction: Transaction,
        tx_metrics: &TransactionExecutionMetrics,
    ) -> Self {
        let execution_metrics = ExecutionMetrics::from_tx_metrics(tx_metrics);
        let writes_metrics = DeduplicatedWritesMetrics::from_tx_metrics(tx_metrics);
        Self {
            execution_metrics,
            cumulative_size: extractors::encoded_transaction_size(transaction),
            writes_metrics,
        }
    }
}
