use ola_basic_types::{bytes8::Bytes8, Address, H256};

use crate::{vm_trace::Call, Transaction};

use self::tx_execution_info::{ExecutionMetrics, TxExecutionStatus};

pub mod execute;
pub mod tx_execution_info;

#[derive(Debug, Clone, PartialEq)]
pub struct TransactionExecutionResult {
    pub transaction: Transaction,
    pub hash: H256,
    pub execution_info: ExecutionMetrics,
    pub execution_status: TxExecutionStatus,
    pub call_traces: Vec<Call>,
    pub revert_reason: Option<String>,
}

impl TransactionExecutionResult {
    pub fn call_trace(&self) -> Option<Call> {
        if self.call_traces.is_empty() {
            None
        } else {
            Some(Call::new_high_level(
                self.transaction.execute.calldata.clone(),
                Bytes8(vec![]),
                self.revert_reason.clone(),
                self.call_traces.clone(),
            ))
        }
    }
}

#[derive(Debug, Clone)]
pub struct IncludedTxLocation {
    pub tx_hash: H256,
    pub tx_index_in_miniblock: u32,
    pub tx_initiator_address: Address,
}
