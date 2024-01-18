use ola_types::{
    log::StorageLogQuery,
    tx::tx_execution_info::{TxExecutionStatus, VmExecutionLogs},
    vm_trace::Call,
    U256,
};
use olavm_core::trace::trace::Trace;
use olavm_core::merkle_tree::log::StorageQuery;

use crate::{errors::VmRevertReasonParsingResult, Word};

#[derive(Debug, PartialEq, Default)]
pub struct VmExecutionResult {
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
    pub fn new(storage_queries: &Vec<StorageQuery>) -> Self {
        let storage_logs: Vec<StorageLogQuery> = storage_queries.iter().map(|log| {
            let log_query = log.into();
            StorageLogQuery {
                log_query,
                log_type: log.kind.into(),
            }
        }).collect();
        let total_log_queries_count = storage_logs.len();
        let logs: VmExecutionLogs = VmExecutionLogs {
            storage_logs,
            events: vec![],
            total_log_queries_count,
        };
        Self { logs, revert_reason: None, contracts_used: 0, cycles_used: 0 }
    }
}

#[derive(Debug, Clone)]
pub struct VmTxExecutionResult {
    pub status: TxExecutionStatus,
    pub result: VmPartialExecutionResult,
    pub ret: Vec<u8>,
    pub trace: Trace,
    pub call_traces: Vec<Call>,
    // Gas refunded to the user at the end of the transaction
    pub gas_refunded: u32,
    // Gas proposed by the operator to be refunded, before the postOp call.
    // This value is needed to correctly recover memory of the bootloader.
    pub operator_suggested_refund: u32,
}
