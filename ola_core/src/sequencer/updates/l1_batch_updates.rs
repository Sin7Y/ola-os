use ola_types::{
    priority_op_onchain_data::PriorityOpOnchainData,
    tx::{tx_execution_info::ExecutionMetrics, TransactionExecutionResult},
};

use super::miniblock_updates::MiniblockUpdates;

#[derive(Debug, Clone, PartialEq)]
pub struct L1BatchUpdates {
    pub executed_transactions: Vec<TransactionExecutionResult>,
    pub priority_ops_onchain_data: Vec<PriorityOpOnchainData>,
    pub block_execution_metrics: ExecutionMetrics,
    pub txs_encoding_size: usize,
}

impl L1BatchUpdates {
    pub(crate) fn new() -> Self {
        Self {
            executed_transactions: Default::default(),
            priority_ops_onchain_data: Default::default(),
            block_execution_metrics: Default::default(),
            txs_encoding_size: 0,
        }
    }

    pub(crate) fn extend_from_sealed_miniblock(&mut self, miniblock_updates: MiniblockUpdates) {
        self.executed_transactions
            .extend(miniblock_updates.executed_transactions);

        self.block_execution_metrics += miniblock_updates.block_execution_metrics;
        self.txs_encoding_size += miniblock_updates.txs_encoding_size;
    }
}
