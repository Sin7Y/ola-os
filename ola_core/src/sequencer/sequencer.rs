use anyhow::Context as _;
use std::{convert::Infallible, time::Duration};

use ola_types::{
    block::MiniblockReexecuteData, proofs::PrepareBasicCircuitsJob,
    protocol_version::ProtocolUpgradeTx, storage_writes_deduplicator::StorageWritesDeduplicator,
    tx::tx_execution_info::TxExecutionStatus, Transaction,
};

use ola_vm::errors::TxRevertReason;
use tokio::sync::watch;

use crate::sequencer::{
    batch_executor::TxExecutionResult, extractors, io::PendingBatchData,
    types::ExecutionMetricsForCriteria, updates::UpdatesManager, SealData,
};
use olaos_object_store::ObjectStore;
use std::sync::Arc;

use super::{
    batch_executor::{BatchExecutorHandle, L1BatchExecutorBuilder},
    io::{L1BatchParams, SequencerIO},
    seal_criteria::{SealManager, SealResolution},
};

pub(super) const POLL_WAIT_DURATION: Duration = Duration::from_secs(1);

/// Structure used to indicate that task cancellation was requested.
#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("canceled")]
    Canceled,
    #[error(transparent)]
    Fatal(#[from] anyhow::Error),
}

impl Error {
    fn context(self, msg: &'static str) -> Self {
        match self {
            Self::Canceled => Self::Canceled,
            Self::Fatal(err) => Self::Fatal(err.context(msg)),
        }
    }
}

#[derive(Debug)]
pub struct OlaSequencer {
    stop_receiver: watch::Receiver<bool>,
    io: Box<dyn SequencerIO>,
    batch_executor_base: Box<dyn L1BatchExecutorBuilder>,
    sealer: SealManager,
    object_store: Arc<dyn ObjectStore>,
}

impl OlaSequencer {
    pub fn new(
        stop_receiver: watch::Receiver<bool>,
        io: Box<dyn SequencerIO>,
        batch_executor_base: Box<dyn L1BatchExecutorBuilder>,
        sealer: SealManager,
        object_store: Arc<dyn ObjectStore>,
    ) -> Self {
        OlaSequencer {
            stop_receiver,
            io,
            batch_executor_base,
            sealer,
            object_store,
        }
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        match self.run_inner().await {
            Ok(_) => unreachable!(),
            Err(Error::Fatal(err)) => Err(err).context("state_keeper failed"),
            Err(cancelled) => {
                olaos_logs::info!(
                    "Stop signal received, sequencer is shutting down. {:?}",
                    cancelled.context("sequencer cancelled")
                );
                Ok(())
            }
        }
    }

    async fn run_inner(&mut self) -> Result<Infallible, Error> {
        olaos_logs::info!(
            "Starting sequencer. Next l1 batch to seal: {}, Next miniblock to seal: {}",
            self.io.current_l1_batch_number(),
            self.io.current_miniblock_number()
        );

        // Re-execute pending batch if it exists. Otherwise, initialize a new batch.
        let PendingBatchData {
            params,
            pending_miniblocks,
        } = match self.io.load_pending_batch().await {
            Some(params) => {
                olaos_logs::info!(
                    "There exists a pending batch consisting of {} miniblocks, the first one is {}",
                    params.pending_miniblocks.len(),
                    params
                        .pending_miniblocks
                        .first()
                        .map(|miniblock| miniblock.number)
                        .expect("Empty pending block represented as Some")
                );
                params
            }
            None => {
                olaos_logs::info!("There is no open pending batch, starting a new empty batch");
                PendingBatchData {
                    params: self.wait_for_new_batch_params().await?,
                    pending_miniblocks: Vec::new(),
                }
            }
        };

        let mut l1_batch_params = params;
        let mut updates_manager = UpdatesManager::new(
            &l1_batch_params.context_mode,
            l1_batch_params.base_system_contracts.hashes(),
            l1_batch_params.protocol_version,
        );

        let previous_batch_protocol_version = self.io.load_previous_batch_version_id().await;

        // TODO: add protocol upgrade logic
        let _version_changed = match previous_batch_protocol_version {
            Some(previous_batch_protocol_version) => {
                l1_batch_params.protocol_version != previous_batch_protocol_version
            }
            // None is only the case for old blocks. Match will be removed when migration will be done.
            None => false,
        };

        let mut protocol_upgrade_tx: Option<ProtocolUpgradeTx> = None;

        let mut batch_executor = self
            .batch_executor_base
            .init_batch(l1_batch_params.clone())
            .await;
        self.restore_state(&batch_executor, &mut updates_manager, pending_miniblocks)
            .await?;

        loop {
            self.check_if_cancelled()?;

            // This function will run until the batch can be sealed.
            self.process_l1_batch(&batch_executor, &mut updates_manager, protocol_upgrade_tx)
                .await?;

            olaos_logs::info!("start finish current batch");

            // Finish current batch.
            if !updates_manager.miniblock.executed_transactions.is_empty() {
                olaos_logs::info!("executed transactions not empty");

                self.io.seal_miniblock(&updates_manager).await;
                // We've sealed the miniblock that we had, but we still need to setup the timestamp
                // for the fictive miniblock.
                let fictive_miniblock_timestamp = self
                    .wait_for_new_miniblock_params(updates_manager.miniblock.timestamp)
                    .await?;
                updates_manager.push_miniblock(fictive_miniblock_timestamp);
            }

            olaos_logs::info!("finish batch {:?}", l1_batch_params.block_number());

            let (block_result, tx_traces) = batch_executor
                .finish_batch(updates_manager.pending_executed_transactions_len() as u32)
                .await;

            // Save transaction traces into object store indexed by batch number.
            let l1_batch_number = self.io.current_l1_batch_number();
            let witness = PrepareBasicCircuitsJob {
                tx_trace: Some(tx_traces),
                storage: None,
                pre_root_hash: None,
                root_hash: None,
            };
            let object_key = self
                .object_store
                .put(l1_batch_number, &witness)
                .await
                .expect(&format!(
                    "Failed to put partial prepare_witness into object store in batch {}",
                    l1_batch_number
                ));
            olaos_logs::info!(
                "Saved witnesses for L1 batch #{l1_batch_number} to object storage at `{object_key}`"
            );

            let sealed_batch_protocol_version = updates_manager.protocol_version();
            self.io
                .seal_l1_batch(
                    block_result,
                    updates_manager,
                    l1_batch_params.context_mode.inner_block_context(),
                )
                .await;

            // Start the new batch.
            l1_batch_params = self.wait_for_new_batch_params().await?;

            olaos_logs::info!("get new batch params {:?}", l1_batch_params.block_number());

            updates_manager = UpdatesManager::new(
                &l1_batch_params.context_mode,
                l1_batch_params.base_system_contracts.hashes(),
                l1_batch_params.protocol_version,
            );
            batch_executor = self
                .batch_executor_base
                .init_batch(l1_batch_params.clone())
                .await;

            let version_changed = l1_batch_params.protocol_version != sealed_batch_protocol_version;
            protocol_upgrade_tx = if version_changed {
                self.io
                    .load_upgrade_tx(l1_batch_params.protocol_version)
                    .await
            } else {
                None
            };
        }
    }

    fn check_if_cancelled(&self) -> Result<(), Error> {
        if *self.stop_receiver.borrow() {
            return Err(Error::Canceled);
        }
        Ok(())
    }

    async fn wait_for_new_batch_params(&mut self) -> Result<L1BatchParams, Error> {
        let params = loop {
            if let Some(params) = self.io.wait_for_new_batch_params(POLL_WAIT_DURATION).await {
                break params;
            }
            self.check_if_cancelled()?;
        };
        olaos_logs::info!("get new params for batch {:?}", params.block_number());
        Ok(params)
    }

    #[olaos_logs::instrument(skip(self))]
    async fn wait_for_new_miniblock_params(
        &mut self,
        prev_miniblock_timestamp: u64,
    ) -> Result<u64, Error> {
        let params = loop {
            if let Some(params) = self
                .io
                .wait_for_new_miniblock_params(POLL_WAIT_DURATION, prev_miniblock_timestamp)
                .await
            {
                break params;
            }
            self.check_if_cancelled()?;
        };
        Ok(params)
    }

    #[olaos_logs::instrument(skip(self))]
    async fn restore_state(
        &mut self,
        batch_executor: &BatchExecutorHandle,
        updates_manager: &mut UpdatesManager,
        miniblocks_to_reexecute: Vec<MiniblockReexecuteData>,
    ) -> Result<(), Error> {
        if miniblocks_to_reexecute.is_empty() {
            return Ok(());
        }

        let mut tx_index_in_l1_batch = 0;

        for (index, miniblock) in miniblocks_to_reexecute.into_iter().enumerate() {
            // Push any non-first miniblock to updates manager. The first one was pushed when `updates_manager` was initialized.
            if index > 0 {
                updates_manager.push_miniblock(miniblock.timestamp);
            }

            let miniblock_number = miniblock.number;
            olaos_logs::info!(
                "Starting to reexecute transactions from sealed miniblock {}",
                miniblock_number
            );
            for tx in miniblock.txs {
                let result = batch_executor
                    .execute_tx(tx.clone(), tx_index_in_l1_batch)
                    .await;
                let TxExecutionResult::Success {
                    tx_result,
                    tx_metrics,
                    ..
                } = result
                else {
                    panic!(
                        "Re-executing stored tx failed. Tx: {:?}. Err: {:?}",
                        tx,
                        result.err()
                    );
                };

                let ExecutionMetricsForCriteria {
                    execution_metrics: tx_execution_metrics,
                } = tx_metrics;

                let exec_result_status = tx_result.status;

                let tx_hash = tx.hash();
                let initiator_account = tx.initiator_account();
                updates_manager.extend_from_executed_transaction(
                    tx,
                    *tx_result,
                    tx_execution_metrics,
                );
                olaos_logs::info!(
                    "Finished re-executing tx {tx_hash} by {initiator_account}, \
                     #{idx_in_l1_batch} in L1 batch {l1_batch_number}, #{idx_in_miniblock} in miniblock {miniblock_number}); \
                     status: {exec_result_status:?}. \
                     Tx execution metrics: {tx_execution_metrics:?}, block execution metrics: {block_execution_metrics:?}",
                    idx_in_l1_batch = updates_manager.pending_executed_transactions_len(),
                    l1_batch_number = self.io.current_l1_batch_number().0,
                    idx_in_miniblock = updates_manager.miniblock.executed_transactions.len(),
                    block_execution_metrics = updates_manager.pending_execution_metrics()
                );
                tx_index_in_l1_batch += 1;
            }
        }

        // We've processed all the miniblocks, and right now we're initializing the next *actual* miniblock.
        let new_timestamp = self
            .wait_for_new_miniblock_params(updates_manager.miniblock.timestamp)
            .await?;
        updates_manager.push_miniblock(new_timestamp);

        Ok(())
    }

    #[olaos_logs::instrument(skip_all)]
    async fn process_l1_batch(
        &mut self,
        batch_executor: &BatchExecutorHandle,
        updates_manager: &mut UpdatesManager,
        protocol_upgrade_tx: Option<ProtocolUpgradeTx>,
    ) -> Result<(), Error> {
        if let Some(protocol_upgrade_tx) = protocol_upgrade_tx {
            self.process_upgrade_tx(batch_executor, updates_manager, protocol_upgrade_tx)
                .await;
        }

        loop {
            self.check_if_cancelled()?;
            if self
                .sealer
                .should_seal_l1_batch_unconditionally(updates_manager)
            {
                olaos_logs::info!(
                    "L1 batch #{} should be sealed unconditionally as per sealing rules",
                    self.io.current_l1_batch_number()
                );
                return Ok(());
            }

            if self.sealer.should_seal_miniblock(updates_manager) {
                olaos_logs::info!(
                    "Miniblock #{} (L1 batch #{}) should be sealed as per sealing rules",
                    self.io.current_miniblock_number(),
                    self.io.current_l1_batch_number()
                );
                self.io.seal_miniblock(updates_manager).await;

                let new_timestamp = self
                    .wait_for_new_miniblock_params(updates_manager.miniblock.timestamp)
                    .await?;
                olaos_logs::info!(
                    "Initialized new miniblock #{} (L1 batch #{}) with timestamp {}",
                    self.io.current_miniblock_number(),
                    self.io.current_l1_batch_number(),
                    extractors::display_timestamp(new_timestamp)
                );
                updates_manager.push_miniblock(new_timestamp);
            }

            let Some(tx) = self.io.wait_for_next_tx(POLL_WAIT_DURATION).await else {
                olaos_logs::info!("No new transactions. Waiting!");
                continue;
            };

            let tx_hash = tx.hash();
            let (seal_resolution, exec_result) = self
                .process_one_tx(batch_executor, updates_manager, tx.clone())
                .await;

            match &seal_resolution {
                SealResolution::NoSeal | SealResolution::IncludeAndSeal => {
                    let TxExecutionResult::Success {
                        tx_result,
                        tx_metrics,
                        ..
                    } = exec_result
                    else {
                        unreachable!(
                            "Tx inclusion seal resolution must be a result of a successful tx execution",
                        );
                    };
                    let ExecutionMetricsForCriteria {
                        execution_metrics: tx_execution_metrics,
                    } = tx_metrics;
                    updates_manager.extend_from_executed_transaction(
                        tx,
                        *tx_result,
                        tx_execution_metrics,
                    );
                }
                SealResolution::ExcludeAndSeal => {
                    self.io.rollback(tx).await;
                }
                SealResolution::Unexecutable(reason) => {
                    self.io.reject(&tx, reason).await;
                }
            };

            if seal_resolution.should_seal() {
                olaos_logs::info!(
                    "L1 batch #{} should be sealed with resolution {seal_resolution:?} after executing \
                     transaction {tx_hash}",
                    self.io.current_l1_batch_number()
                );
                return Ok(());
            }
        }
    }

    #[olaos_logs::instrument(skip_all)]
    async fn process_upgrade_tx(
        &mut self,
        batch_executor: &BatchExecutorHandle,
        updates_manager: &mut UpdatesManager,
        protocol_upgrade_tx: ProtocolUpgradeTx,
    ) {
        // Sanity check: protocol upgrade tx must be the first one in the batch.
        assert_eq!(updates_manager.pending_executed_transactions_len(), 0);

        let tx: Transaction = protocol_upgrade_tx.into();
        let (seal_resolution, exec_result) = self
            .process_one_tx(batch_executor, updates_manager, tx.clone())
            .await;

        match &seal_resolution {
            SealResolution::NoSeal | SealResolution::IncludeAndSeal => {
                let TxExecutionResult::Success {
                    tx_result,
                    tx_metrics,
                    ..
                } = exec_result
                else {
                    panic!(
                        "Tx inclusion seal resolution must be a result of a successful tx execution",
                    );
                };

                // Despite success of upgrade transaction is not enforced by protocol,
                // we panic here because failed upgrade tx is not intended in any case.
                if tx_result.status != TxExecutionStatus::Success {
                    panic!("Failed upgrade tx {:?}", tx.hash());
                }

                let ExecutionMetricsForCriteria {
                    execution_metrics: tx_execution_metrics,
                    ..
                } = tx_metrics;
                updates_manager.extend_from_executed_transaction(
                    tx,
                    *tx_result,
                    tx_execution_metrics,
                );
            }
            SealResolution::ExcludeAndSeal => {
                unreachable!("First tx in batch cannot result into `ExcludeAndSeal`");
            }
            SealResolution::Unexecutable(reason) => {
                panic!(
                    "Upgrade transaction {:?} is unexecutable: {}",
                    tx.hash(),
                    reason
                );
            }
        };
    }

    #[olaos_logs::instrument(skip_all)]
    async fn process_one_tx(
        &mut self,
        batch_executor: &BatchExecutorHandle,
        updates_manager: &mut UpdatesManager,
        tx: Transaction,
    ) -> (SealResolution, TxExecutionResult) {
        let tx_index_in_l1_batch = updates_manager.pending_executed_transactions_len() as u32;
        let exec_result = batch_executor
            .execute_tx(tx.clone(), tx_index_in_l1_batch)
            .await;
        olaos_logs::info!(
            "tx {:?} executed by batch_executor, exec_result with error {:?}",
            tx.hash(),
            exec_result.err()
        );
        let resolution = match &exec_result {
            TxExecutionResult::BootloaderOutOfGasForTx => SealResolution::ExcludeAndSeal,
            TxExecutionResult::BootloaderOutOfGasForBlockTip => SealResolution::ExcludeAndSeal,
            TxExecutionResult::RejectedByVm { rejection_reason } => match rejection_reason {
                TxRevertReason::NotEnoughGasProvided => SealResolution::ExcludeAndSeal,
                _ => SealResolution::Unexecutable(rejection_reason.to_string()),
            },
            TxExecutionResult::Success {
                tx_result,
                tx_metrics,
                entrypoint_dry_run_metrics,
                entrypoint_dry_run_result,
                ..
            } => {
                let tx_execution_status = tx_result.status;
                let ExecutionMetricsForCriteria {
                    execution_metrics: tx_execution_metrics,
                } = *tx_metrics;

                olaos_logs::info!(
                    "finished tx {:?} by {:?} (is_l1: {}) (#{} in l1 batch {}) (#{} in miniblock {}) \
                    status: {:?}. \
                    Tx execution metrics: {:?}, block execution metrics: {:?}",
                    tx.hash(),
                    tx.initiator_account(),
                    false,
                    updates_manager.pending_executed_transactions_len() + 1,
                    self.io.current_l1_batch_number().0,
                    updates_manager.miniblock.executed_transactions.len() + 1,
                    self.io.current_miniblock_number().0,
                    tx_execution_status,
                    &tx_execution_metrics,
                    updates_manager.pending_execution_metrics() + tx_execution_metrics,
                );

                let ExecutionMetricsForCriteria {
                    execution_metrics: finish_block_execution_metrics,
                } = *entrypoint_dry_run_metrics;

                let encoding_len = extractors::encoded_transaction_size(tx);

                let logs_to_apply = tx_result.result.logs.storage_logs.iter();
                let logs_to_apply =
                    logs_to_apply.chain(&entrypoint_dry_run_result.logs.storage_logs);
                let block_writes_metrics = updates_manager
                    .storage_writes_deduplicator
                    .apply_and_rollback(logs_to_apply.clone());

                let tx_writes_metrics =
                    StorageWritesDeduplicator::apply_on_empty_state(logs_to_apply);

                let tx_data = SealData {
                    execution_metrics: tx_execution_metrics + finish_block_execution_metrics,
                    cumulative_size: encoding_len,
                    writes_metrics: tx_writes_metrics,
                };
                let block_data = SealData {
                    execution_metrics: tx_data.execution_metrics
                        + updates_manager.pending_execution_metrics(),
                    cumulative_size: tx_data.cumulative_size
                        + updates_manager.pending_txs_encoding_size(),
                    writes_metrics: block_writes_metrics,
                };
                self.sealer.should_seal_l1_batch(
                    self.io.current_l1_batch_number().0,
                    updates_manager.batch_timestamp() as u128 * 1_000,
                    updates_manager.pending_executed_transactions_len() + 1,
                    &block_data,
                    &tx_data,
                )
            }
        };
        olaos_logs::info!("process resolution {:?}", resolution);
        (resolution, exec_result)
    }
}
