use ola_types::fee::TransactionExecutionMetrics;
use ola_vm::vm::VmExecutionResult;

pub(super) fn collect_tx_execution_metrics(
    _contracts_deployed: u16,
    _result: &VmExecutionResult,
) -> TransactionExecutionMetrics {
    // TODO:
    TransactionExecutionMetrics::default()
}
