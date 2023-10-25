use ola_config::{
    chain::MempoolConfig, constants::crypto::MAX_TXS_IN_BLOCK, contracts::ContractsConfig,
    database::DBConfig, sequencer::SequencerConfig,
};
use ola_dal::connection::ConnectionPool;
use ola_types::{
    fee::TransactionExecutionMetrics,
    tx::tx_execution_info::{DeduplicatedWritesMetrics, ExecutionMetrics},
    Transaction,
};
use tokio::sync::watch;

use crate::sequencer::{
    batch_executor::MainBatchExecutorBuilder, io::mempool::MempoolIO, seal_criteria::SealManager,
};

use self::{io::MiniblockSealerHandle, sequencer::OlaSequencer, types::MempoolGuard};

pub mod batch_executor;
pub mod extractors;
pub mod io;
pub mod mempool_actor;
pub mod seal_criteria;
pub mod sequencer;
pub mod types;
pub mod updates;

#[derive(Debug, Default)]
pub struct SealData {
    pub(super) execution_metrics: ExecutionMetrics,
    pub(super) cumulative_size: usize,
    pub(super) writes_metrics: DeduplicatedWritesMetrics,
}

impl SealData {
    pub(crate) fn for_transaction(
        transaction: Transaction,
        tx_metrics: &TransactionExecutionMetrics,
    ) -> Self {
        let execution_metrics = ExecutionMetrics::from_tx_metrics(tx_metrics);
        let writes_metrics = DeduplicatedWritesMetrics::from_tx_metrics(tx_metrics);
        Self {
            execution_metrics,
            cumulative_size: extractors::encoded_transaction_size(transaction),
            writes_metrics,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_sequencer(
    contracts_config: &ContractsConfig,
    sequencer_config: SequencerConfig,
    db_config: &DBConfig,
    mempool_config: &MempoolConfig,
    pool: ConnectionPool,
    mempool: MempoolGuard,
    miniblock_sealer_handle: MiniblockSealerHandle,
    stop_receiver: watch::Receiver<bool>,
) -> OlaSequencer {
    assert!(
        sequencer_config.transaction_slots <= MAX_TXS_IN_BLOCK,
        "Configured transaction_slots ({}) must be lower than the bootloader constant MAX_TXS_IN_BLOCK={}",
        sequencer_config.transaction_slots,
        MAX_TXS_IN_BLOCK
    );

    let batch_executor_base = MainBatchExecutorBuilder::new(
        db_config.sequencer_db_path.clone(),
        pool.clone(),
        sequencer_config.save_call_traces,
    );

    let io = MempoolIO::new(
        mempool,
        miniblock_sealer_handle,
        pool,
        &sequencer_config,
        mempool_config.delay_interval(),
        contracts_config.l2_erc20_bridge_addr,
    )
    .await;

    let sealer = SealManager::new(sequencer_config);
    OlaSequencer::new(
        stop_receiver,
        Box::new(io),
        Box::new(batch_executor_base),
        sealer,
    )
}
