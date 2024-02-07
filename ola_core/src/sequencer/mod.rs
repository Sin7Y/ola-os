use ola_config::{
    chain::MempoolConfig, constants::crypto::MAX_TXS_IN_BLOCK, contracts::ContractsConfig,
    database::DBConfig, sequencer::SequencerConfig,
};
use ola_dal::connection::ConnectionPool;
use ola_types::tx::tx_execution_info::{DeduplicatedWritesMetrics, ExecutionMetrics};
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

#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_sequencer(
    _contracts_config: &ContractsConfig,
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
        db_config.merkle_tree.path.clone(),
        pool.clone(),
        sequencer_config.save_call_traces,
    );

    let io = MempoolIO::new(
        mempool,
        miniblock_sealer_handle,
        pool,
        &sequencer_config,
        mempool_config.delay_interval(),
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
