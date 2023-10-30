use std::collections::HashMap;

use ola_types::{
    events::{extract_bytecodes_marked_as_known, VmEvent},
    log::StorageLogQuery,
    tx::{
        tx_execution_info::{ExecutionMetrics, VmExecutionLogs},
        TransactionExecutionResult,
    },
    Transaction, H256,
};
use ola_utils::bytecode::{hash_bytecode, CompressedBytecodeInfo};
use ola_vm::vm::VmTxExecutionResult;

use crate::sequencer::extractors;

#[derive(Debug, Clone, PartialEq)]
pub struct MiniblockUpdates {
    pub executed_transactions: Vec<TransactionExecutionResult>,
    // pub events: Vec<VmEvent>,
    pub storage_logs: Vec<StorageLogQuery>,
    pub new_factory_deps: HashMap<H256, Vec<u8>>,
    pub block_execution_metrics: ExecutionMetrics,
    pub txs_encoding_size: usize,
    pub timestamp: u64,
}

impl MiniblockUpdates {
    pub(crate) fn new(timestamp: u64) -> Self {
        Self {
            executed_transactions: vec![],
            // events: vec![],
            storage_logs: vec![],
            new_factory_deps: HashMap::new(),
            block_execution_metrics: ExecutionMetrics::default(),
            txs_encoding_size: 0,
            timestamp,
        }
    }

    pub(crate) fn extend_from_executed_transaction(
        &mut self,
        tx: Transaction,
        tx_execution_result: VmTxExecutionResult,
        execution_metrics: ExecutionMetrics,
        compressed_bytecodes: Vec<CompressedBytecodeInfo>,
    ) {
        // Get bytecode hashes that were marked as known
        let saved_factory_deps =
            extract_bytecodes_marked_as_known(&tx_execution_result.result.logs.events);

        // Get transaction factory deps
        let factory_deps = tx.execute.factory_deps.as_deref().unwrap_or_default();
        let tx_factory_deps: HashMap<_, _> = factory_deps
            .iter()
            .map(|bytecode| (hash_bytecode(bytecode), bytecode))
            .collect();

        // Save all bytecodes that were marked as known on the bootloader
        let known_bytecodes = saved_factory_deps.into_iter().map(|bytecode_hash| {
            let bytecode = tx_factory_deps.get(&bytecode_hash).unwrap_or_else(|| {
                panic!(
                    "Failed to get factory deps on tx: bytecode hash: {:?}, tx hash: {}",
                    bytecode_hash,
                    tx.hash()
                )
            });
            (bytecode_hash, bytecode.to_vec())
        });
        self.new_factory_deps.extend(known_bytecodes);

        // self.events.extend(tx_execution_result.result.logs.events);
        self.storage_logs
            .extend(tx_execution_result.result.logs.storage_logs);

        self.block_execution_metrics += execution_metrics;
        self.txs_encoding_size += extractors::encoded_transaction_size(tx.clone());

        self.executed_transactions.push(TransactionExecutionResult {
            hash: tx.hash(),
            transaction: tx,
            execution_info: execution_metrics,
            execution_status: tx_execution_result.status,
            compressed_bytecodes,
            call_traces: tx_execution_result.call_traces,
            revert_reason: tx_execution_result
                .result
                .revert_reason
                .map(|reason| reason),
        });
    }

    pub(crate) fn extend_from_fictive_transaction(&mut self, vm_execution_logs: VmExecutionLogs) {
        // self.events.extend(vm_execution_logs.events);
        self.storage_logs.extend(vm_execution_logs.storage_logs);
    }
}