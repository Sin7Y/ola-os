use ola_types::{
    events::VmEvent,
    log::{LogQuery, StorageLogQuery},
    tx::tx_execution_info::{TxExecutionStatus, VmExecutionLogs},
    U256,
};
use olavm_core::{
    trace::exe_trace::TxExeTrace,
    vm::{hardware::StorageAccessLog, types::Event},
};

use crate::{errors::VmRevertReasonParsingResult, Word};

#[derive(Debug, PartialEq, Default)]
pub struct VmExecutionResult {
    pub events: Vec<VmEvent>,
    pub storage_log_queries: Vec<StorageLogQuery>,
    pub used_contract_hashes: Vec<U256>,
    pub return_data: Vec<Word>,
    pub contracts_used: usize,
    pub cycles_used: u32,
    pub revert_reason: Option<VmRevertReasonParsingResult>,
}

#[derive(Debug, PartialEq)]
pub struct VmBlockResult {
    /// Result for the whole block execution.
    pub full_result: VmExecutionResult,
    /// Result for the block tip execution.
    pub block_tip_result: VmPartialExecutionResult,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct VmPartialExecutionResult {
    pub logs: VmExecutionLogs,
    pub revert_reason: Option<String>,
    pub contracts_used: usize,
    pub cycles_used: u32,
}

impl VmPartialExecutionResult {
    pub fn from_storage_events(
        storage_access_logs: &Vec<StorageAccessLog>,
        events: &Vec<Event>,
        tx_index_in_l1_batch: u32,
    ) -> Self {
        let storage_logs: Vec<StorageLogQuery> = storage_access_logs
            .iter()
            .map(|log| {
                let mut log_query: LogQuery = log.into();
                log_query.tx_number_in_block = tx_index_in_l1_batch as u16;
                StorageLogQuery {
                    log_query,
                    log_type: log.kind.into(),
                }
            })
            .collect();
        let total_log_queries_count = storage_logs.len();
        let vm_events: Vec<VmEvent> = events.iter().map(|e| e.into()).collect();
        let logs: VmExecutionLogs = VmExecutionLogs {
            storage_logs,
            events: vm_events,
            total_log_queries_count,
        };
        Self {
            logs,
            revert_reason: None,
            contracts_used: 0,
            cycles_used: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VmTxExeResult {
    pub status: TxExecutionStatus,
    pub result: VmPartialExecutionResult,
    pub trace: TxExeTrace,
    // Gas refunded to the user at the end of the transaction
    pub gas_refunded: u32,
    // Gas proposed by the operator to be refunded, before the postOp call.
    // This value is needed to correctly recover memory of the bootloader.
    pub operator_suggested_refund: u32,
}
