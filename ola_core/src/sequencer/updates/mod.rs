use ola_contracts::BaseSystemContractsHashes;
use ola_types::{
    protocol_version::ProtocolVersionId,
    storage_writes_deduplicator::StorageWritesDeduplicator,
    tx::tx_execution_info::{ExecutionMetrics, VmExecutionLogs},
    L1BatchNumber, MiniblockNumber, Transaction,
};
use ola_vm::{vm::VmTxExecutionResult, vm_with_bootloader::BlockContextMode};

use self::{l1_batch_updates::L1BatchUpdates, miniblock_updates::MiniblockUpdates};

pub mod l1_batch_updates;
pub mod miniblock_updates;

#[derive(Debug)]
pub(crate) struct MiniblockSealCommand {
    pub l1_batch_number: L1BatchNumber,
    pub miniblock_number: MiniblockNumber,
    pub miniblock: MiniblockUpdates,
    pub first_tx_index: usize,
    pub base_system_contracts_hashes: BaseSystemContractsHashes,
    pub protocol_version: ProtocolVersionId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdatesManager {
    batch_timestamp: u64,
    base_system_contract_hashes: BaseSystemContractsHashes,
    protocol_version: ProtocolVersionId,
    pub l1_batch: L1BatchUpdates,
    pub miniblock: MiniblockUpdates,
    pub storage_writes_deduplicator: StorageWritesDeduplicator,
}

impl UpdatesManager {
    #[olaos_logs::instrument]
    pub(crate) fn new(
        block_context: &BlockContextMode,
        base_system_contract_hashes: BaseSystemContractsHashes,
        protocol_version: ProtocolVersionId,
    ) -> Self {
        let batch_timestamp = block_context.timestamp();
        Self {
            batch_timestamp,
            protocol_version,
            base_system_contract_hashes,
            l1_batch: L1BatchUpdates::new(),
            miniblock: MiniblockUpdates::new(batch_timestamp),
            storage_writes_deduplicator: StorageWritesDeduplicator::new(),
        }
    }

    #[olaos_logs::instrument(skip(self))]
    pub(crate) fn push_miniblock(&mut self, new_miniblock_timestamp: u64) {
        let new_miniblock_updates = MiniblockUpdates::new(new_miniblock_timestamp);
        let old_miniblock_updates = std::mem::replace(&mut self.miniblock, new_miniblock_updates);

        self.l1_batch
            .extend_from_sealed_miniblock(old_miniblock_updates);
    }

    #[olaos_logs::instrument(skip_all, fields(execution_metrics))]
    pub(crate) fn extend_from_executed_transaction(
        &mut self,
        tx: Transaction,
        tx_execution_result: VmTxExecutionResult,
        execution_metrics: ExecutionMetrics,
    ) {
        self.storage_writes_deduplicator
            .apply(&tx_execution_result.result.logs.storage_logs);
        self.miniblock
            .extend_from_executed_transaction(tx, tx_execution_result, execution_metrics);
    }

    #[olaos_logs::instrument(skip_all)]
    pub(crate) fn extend_from_fictive_transaction(&mut self, vm_execution_logs: VmExecutionLogs) {
        self.storage_writes_deduplicator
            .apply(&vm_execution_logs.storage_logs);
        self.miniblock
            .extend_from_fictive_transaction(vm_execution_logs.clone());
    }

    #[olaos_logs::instrument(skip(self))]
    pub(crate) fn seal_miniblock_command(
        &self,
        l1_batch_number: L1BatchNumber,
        miniblock_number: MiniblockNumber,
    ) -> MiniblockSealCommand {
        MiniblockSealCommand {
            l1_batch_number,
            miniblock_number,
            miniblock: self.miniblock.clone(),
            first_tx_index: self.l1_batch.executed_transactions.len(),
            base_system_contracts_hashes: self.base_system_contract_hashes,
            protocol_version: self.protocol_version,
        }
    }

    pub(crate) fn protocol_version(&self) -> ProtocolVersionId {
        self.protocol_version
    }

    pub(crate) fn batch_timestamp(&self) -> u64 {
        self.batch_timestamp
    }

    pub(crate) fn base_system_contract_hashes(&self) -> BaseSystemContractsHashes {
        self.base_system_contract_hashes
    }

    pub(crate) fn pending_executed_transactions_len(&self) -> usize {
        self.l1_batch.executed_transactions.len() + self.miniblock.executed_transactions.len()
    }

    pub(crate) fn pending_execution_metrics(&self) -> ExecutionMetrics {
        self.l1_batch.block_execution_metrics + self.miniblock.block_execution_metrics
    }

    pub(crate) fn pending_txs_encoding_size(&self) -> usize {
        self.l1_batch.txs_encoding_size + self.miniblock.txs_encoding_size
    }
}
