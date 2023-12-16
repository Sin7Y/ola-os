use std::{fmt, time::Instant};

use async_trait::async_trait;
use ola_dal::connection::ConnectionPool;
use ola_state::rocksdb::RocksdbStorage;
use ola_types::Transaction;
use ola_vm::{
    errors::TxRevertReason,
    vm::{VmBlockResult, VmExecutionResult, VmPartialExecutionResult, VmTxExecutionResult},
};
// use olavm_core::vm::transaction::init_tx_context;
use tempfile::TempDir;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use super::{io::L1BatchParams, types::ExecutionMetricsForCriteria};
use zk_vm::OlaVM;
#[derive(Debug)]
pub struct BatchExecutorHandle {
    handle: JoinHandle<()>,
    commands: mpsc::Sender<Command>,
}

impl BatchExecutorHandle {
    pub(super) fn new(
        save_call_traces: bool,
        secondary_storage: RocksdbStorage,
        l1_batch_params: L1BatchParams,
    ) -> Self {
        // Since we process `BatchExecutor` commands one-by-one (the next command is never enqueued
        // until a previous command is processed), capacity 1 is enough for the commands channel.
        let (commands_sender, commands_receiver) = mpsc::channel(1);
        let executor = BatchExecutor {
            save_call_traces,
            commands: commands_receiver,
        };

        let handle =
            tokio::task::spawn_blocking(move || executor.run(secondary_storage, l1_batch_params));
        Self {
            handle,
            commands: commands_sender,
        }
    }

    pub(super) async fn execute_tx(&self, tx: Transaction) -> TxExecutionResult {
        let (response_sender, response_receiver) = oneshot::channel();
        self.commands
            .send(Command::ExecuteTx(Box::new(tx), response_sender))
            .await
            .unwrap();

        let res = response_receiver.await.unwrap();

        res
    }

    pub(super) async fn finish_batch(self) -> VmBlockResult {
        let (response_sender, response_receiver) = oneshot::channel();
        self.commands
            .send(Command::FinishBatch(response_sender))
            .await
            .unwrap();
        let start = Instant::now();
        let resp = response_receiver.await.unwrap();
        self.handle.await.unwrap();
        resp
    }
}

#[derive(Debug)]
pub(super) enum Command {
    ExecuteTx(Box<Transaction>, oneshot::Sender<TxExecutionResult>),
    RollbackLastTx(oneshot::Sender<()>),
    FinishBatch(oneshot::Sender<VmBlockResult>),
}

#[derive(Debug, Clone)]
pub(crate) enum TxExecutionResult {
    /// Successful execution of the tx and the block tip dry run.
    Success {
        tx_result: Box<VmTxExecutionResult>,
        tx_metrics: ExecutionMetricsForCriteria,
        entrypoint_dry_run_metrics: ExecutionMetricsForCriteria,
        entrypoint_dry_run_result: Box<VmPartialExecutionResult>,
    },
    /// The VM rejected the tx for some reason.
    RejectedByVm { rejection_reason: TxRevertReason },
    /// Bootloader gas limit is not enough to execute the tx.
    BootloaderOutOfGasForTx,
    /// Bootloader gas limit is enough to run the tx but not enough to execute block tip.
    BootloaderOutOfGasForBlockTip,
}

impl TxExecutionResult {
    /// Returns a revert reason if either transaction was rejected or bootloader ran out of gas.
    pub(super) fn err(&self) -> Option<&TxRevertReason> {
        match self {
            Self::Success { .. } => None,
            Self::RejectedByVm { rejection_reason } => Some(rejection_reason),
            Self::BootloaderOutOfGasForTx | Self::BootloaderOutOfGasForBlockTip { .. } => {
                Some(&TxRevertReason::BootloaderOutOfGas)
            }
        }
    }
}

#[async_trait]
pub trait L1BatchExecutorBuilder: 'static + Send + Sync + fmt::Debug {
    async fn init_batch(&self, l1_batch_params: L1BatchParams) -> BatchExecutorHandle;
}

#[derive(Debug, Clone)]
pub struct MainBatchExecutorBuilder {
    sequencer_db_path: String,
    pool: ConnectionPool,
    save_call_traces: bool,
}

impl MainBatchExecutorBuilder {
    pub fn new(sequencer_db_path: String, pool: ConnectionPool, save_call_traces: bool) -> Self {
        Self {
            sequencer_db_path: sequencer_db_path,
            pool,
            save_call_traces,
        }
    }
}

#[async_trait]
impl L1BatchExecutorBuilder for MainBatchExecutorBuilder {
    async fn init_batch(&self, l1_batch_params: L1BatchParams) -> BatchExecutorHandle {
        let mut secondary_storage = RocksdbStorage::new(self.sequencer_db_path.as_ref());
        let mut conn = self.pool.access_storage_tagged("sequencer").await;
        secondary_storage.update_from_postgres(&mut conn).await;
        drop(conn);

        let batch_number = l1_batch_params
            .context_mode
            .inner_block_context()
            .context
            .block_number;

        olaos_logs::info!(
            "Secondary storage for batch {batch_number} initialized, size is {}",
            secondary_storage.estimated_map_size()
        );
        BatchExecutorHandle::new(self.save_call_traces, secondary_storage, l1_batch_params)
    }
}

#[derive(Debug)]
pub(super) struct BatchExecutor {
    save_call_traces: bool,
    commands: mpsc::Receiver<Command>,
}

impl BatchExecutor {
    pub(super) fn run(mut self, secondary_storage: RocksdbStorage, l1_batch_params: L1BatchParams) {
        olaos_logs::info!(
            "Starting executing batch #{}",
            l1_batch_params
                .context_mode
                .inner_block_context()
                .context
                .block_number
        );

        // TODO: @pierre init vm begin
        // let mut storage_view = StorageView::new(&secondary_storage);
        // let block_properties = BlockProperties::new(
        //     self.vm_version,
        //     l1_batch_params.properties.default_aa_code_hash,
        // );

        // let mut vm = match self.vm_gas_limit {
        //     Some(vm_gas_limit) => init_vm_with_gas_limit(
        //         self.vm_version,
        //         &mut oracle_tools,
        //         l1_batch_params.context_mode,
        //         &block_properties,
        //         TxExecutionMode::VerifyExecute,
        //         &l1_batch_params.base_system_contracts,
        //         vm_gas_limit,
        //     ),
        //     None => init_vm(
        //         self.vm_version,
        //         &mut oracle_tools,
        //         l1_batch_params.context_mode,
        //         &block_properties,
        //         TxExecutionMode::VerifyExecute,
        //         &l1_batch_params.base_system_contracts,
        //     ),
        // };

        // TODO: need roscksdb path for storage merkle tree
        let mut vm = OlaVM::new(
            TempDir::new()
                .expect("failed get temporary directory for RocksDB")
                .path(),
            TempDir::new()
                .expect("failed get temporary directory for RocksDB")
                .path(),
            Default::default(), // FIXME: @Pierre
        );
        // TODO: @pierre init vm end

        while let Some(cmd) = self.commands.blocking_recv() {
            match cmd {
                Command::ExecuteTx(tx, resp) => {
                    // FIXME: @pierre
                    let result = self.execute_tx(&tx, &mut vm);
                    let result = TxExecutionResult::BootloaderOutOfGasForBlockTip;
                    resp.send(result).unwrap();
                }
                Command::RollbackLastTx(resp) => {
                    // FIXME: @pierre
                    // self.rollback_last_tx(&mut vm);
                    resp.send(()).unwrap();
                }
                Command::FinishBatch(resp) => {
                    resp.send(self.finish_batch(&mut vm)).unwrap();
                    return;
                }
            }
        }
        // Sequencer can exit because of stop signal, so it's OK to exit mid-batch.
        olaos_logs::info!("Sequencer exited with an unfinished batch");
    }

    fn finish_batch(&self, vm: &mut OlaVM) -> VmBlockResult {
        // FIXME: @pierre
        // vm.execute_till_block_end(BootloaderJobType::BlockPostprocessing)
        VmBlockResult {
            full_result: VmExecutionResult::default(),
            block_tip_result: VmPartialExecutionResult::default(),
        }
    }

    fn execute_tx(&self, tx: &Transaction, vm: &mut OlaVM) {
        // FIXME: @Pierre
        // let res = vm.execute_tx(
        //     GoldilocksField::from_canonical_u64(5),
        //     Address::default(), //tx.address(),
        //     Address::default(), //tx.execute.contract_address,
        //     Vec::new(),         //tx.execute.calldata.clone(),
        // );
    }
}
