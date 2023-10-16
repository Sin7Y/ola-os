use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "result")]
pub struct TransactionExecutionMetrics {
    pub initial_storage_writes: usize,
    pub repeated_storage_writes: usize,
    pub event_topics: u16,
    pub published_bytecode_bytes: usize,
    pub l2_l1_long_messages: usize,
    pub l2_l1_logs: usize,
    pub contracts_used: usize,
    pub contracts_deployed: u16,
    pub vm_events: usize,
    pub storage_logs: usize,
    // it's the sum of storage logs, vm events, l2->l1 logs,
    // and the number of precompile calls
    pub total_log_queries: usize,
    pub cycles_used: u32,
}
