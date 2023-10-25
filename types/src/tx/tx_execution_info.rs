use std::ops::{Add, AddAssign};

use crate::{events::VmEvent, fee::TransactionExecutionMetrics, log::StorageLogQuery};

#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct ExecutionMetrics {
    pub published_bytecode_bytes: usize,
    pub contracts_used: usize,
    pub contracts_deployed: u16,
    pub vm_events: usize,
    pub storage_logs: usize,
    pub total_log_queries: usize,
    pub cycles_used: u32,
}

impl ExecutionMetrics {
    pub fn from_tx_metrics(tx_metrics: &TransactionExecutionMetrics) -> Self {
        Self {
            published_bytecode_bytes: tx_metrics.published_bytecode_bytes,
            contracts_deployed: tx_metrics.contracts_deployed,
            contracts_used: tx_metrics.contracts_used,
            storage_logs: tx_metrics.storage_logs,
            vm_events: tx_metrics.vm_events,
            total_log_queries: tx_metrics.total_log_queries,
            cycles_used: tx_metrics.cycles_used,
        }
    }
}

impl Add for ExecutionMetrics {
    type Output = ExecutionMetrics;

    fn add(self, other: ExecutionMetrics) -> ExecutionMetrics {
        ExecutionMetrics {
            published_bytecode_bytes: self.published_bytecode_bytes
                + other.published_bytecode_bytes,
            contracts_deployed: self.contracts_deployed + other.contracts_deployed,
            contracts_used: self.contracts_used + other.contracts_used,
            vm_events: self.vm_events + other.vm_events,
            storage_logs: self.storage_logs + other.storage_logs,
            total_log_queries: self.total_log_queries + other.total_log_queries,
            cycles_used: self.cycles_used + other.cycles_used,
        }
    }
}

impl AddAssign for ExecutionMetrics {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DeduplicatedWritesMetrics {
    pub initial_storage_writes: usize,
    pub repeated_storage_writes: usize,
}

impl DeduplicatedWritesMetrics {
    pub fn from_tx_metrics(tx_metrics: &TransactionExecutionMetrics) -> Self {
        Self {
            initial_storage_writes: tx_metrics.initial_storage_writes,
            repeated_storage_writes: tx_metrics.repeated_storage_writes,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TxExecutionStatus {
    Success,
    Failure,
}

impl TxExecutionStatus {
    pub fn from_has_failed(has_failed: bool) -> Self {
        if has_failed {
            Self::Failure
        } else {
            Self::Success
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VmExecutionLogs {
    pub storage_logs: Vec<StorageLogQuery>,
    pub events: Vec<VmEvent>,
    pub total_log_queries_count: usize,
}
