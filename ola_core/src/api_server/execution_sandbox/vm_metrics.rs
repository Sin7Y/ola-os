use ola_types::fee::TransactionExecutionMetrics;
use ola_vm::vm::VmExecutionResult;

pub(super) fn collect_tx_execution_metrics(
    contracts_deployed: u16,
    result: &VmExecutionResult,
) -> TransactionExecutionMetrics {
    // TODO:
    TransactionExecutionMetrics::default()
}
