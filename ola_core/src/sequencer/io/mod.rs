use anyhow::Ok;
use async_trait::async_trait;
use ola_contracts::BaseSystemContracts;
use ola_types::{
    block::MiniblockReexecuteData,
    protocol_version::{ProtocolUpgradeTx, ProtocolVersionId},
    L1BatchNumber, MiniblockNumber, Transaction,
};
use ola_vm::{
    vm::VmBlockResult,
    vm_with_bootloader::{BlockContextMode, BlockProperties, DerivedBlockContext},
};
use std::{
    fmt,
    time::{Duration, Instant},
};

use ola_dal::connection::ConnectionPool;
use tokio::sync::{mpsc, oneshot};

use super::updates::{MiniblockSealCommand, UpdatesManager};

pub mod common;
pub mod mempool;
pub mod seal_logic;
pub mod sort_storage_access;

#[derive(Debug)]
pub(crate) struct MiniblockSealer {
    pool: ConnectionPool,
    is_sync: bool,
    // Weak sender handle to get queue capacity stats.
    commands_sender: mpsc::WeakSender<Completable<MiniblockSealCommand>>,
    commands_receiver: mpsc::Receiver<Completable<MiniblockSealCommand>>,
}

impl MiniblockSealer {
    /// Creates a sealer that will use the provided Postgres connection and will have the specified
    /// `command_capacity` for unprocessed sealing commands.
    pub(crate) fn new(
        pool: ConnectionPool,
        mut command_capacity: usize,
    ) -> (Self, MiniblockSealerHandle) {
        let is_sync = command_capacity == 0;
        command_capacity = command_capacity.max(1);

        let (commands_sender, commands_receiver) = mpsc::channel(command_capacity);
        let this = Self {
            pool,
            is_sync,
            commands_sender: commands_sender.downgrade(),
            commands_receiver,
        };
        let handle = MiniblockSealerHandle {
            commands_sender,
            latest_completion_receiver: None,
            is_sync,
        };
        (this, handle)
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        if self.is_sync {
            olaos_logs::info!("Starting synchronous miniblock sealer");
        } else if let Some(sender) = self.commands_sender.upgrade() {
            olaos_logs::info!(
                "Starting async miniblock sealer with queue capacity {}",
                sender.max_capacity()
            );
        } else {
            olaos_logs::warn!("Miniblock sealer not started, since its handle is already dropped");
        }

        // Commands must be processed sequentially: a later miniblock cannot be saved before
        // an earlier one.
        while let Some(completable) = self.next_command().await {
            olaos_logs::info!("Miniblock sealer get a new command: {:?}", completable);
            let mut conn = self.pool.access_storage_tagged("sequencer").await;
            completable.command.seal(&mut conn).await;
            olaos_logs::info!("Miniblock sealer sealed successfully");
            completable.completion_sender.send(()).ok();
            // ^ We don't care whether anyone listens to the processing progress
            olaos_logs::info!("Miniblock sealer send ok to sender");
        }
        Ok(())
    }

    #[olaos_logs::instrument(skip(self))]
    async fn next_command(&mut self) -> Option<Completable<MiniblockSealCommand>> {
        olaos_logs::info!("Polling miniblock seal queue for next command");
        let start = Instant::now();
        let command = self.commands_receiver.recv().await;
        let elapsed = start.elapsed();

        if let Some(completable) = &command {
            olaos_logs::info!(
                "Received command to seal miniblock #{} (polling took {elapsed:?})",
                completable.command.miniblock_number
            );
        }

        command
    }
}

#[derive(Debug)]
struct Completable<T> {
    command: T,
    completion_sender: oneshot::Sender<()>,
}

#[derive(Debug)]
pub(crate) struct MiniblockSealerHandle {
    commands_sender: mpsc::Sender<Completable<MiniblockSealCommand>>,
    latest_completion_receiver: Option<oneshot::Receiver<()>>,
    // If true, `submit()` will wait for the operation to complete.
    is_sync: bool,
}

impl MiniblockSealerHandle {
    const SHUTDOWN_MSG: &'static str = "miniblock sealer unexpectedly shut down";

    #[olaos_logs::instrument(skip(self))]
    pub async fn submit(&mut self, command: MiniblockSealCommand) {
        let miniblock_number = command.miniblock_number;
        olaos_logs::info!(
            "Enqueuing sealing command for miniblock #{miniblock_number} with #{} txs (L1 batch #{})",
            command.miniblock.executed_transactions.len(),
            command.l1_batch_number
        );

        let start = Instant::now();
        let (completion_sender, completion_receiver) = oneshot::channel();
        self.latest_completion_receiver = Some(completion_receiver);
        let command = Completable {
            command,
            completion_sender,
        };

        olaos_logs::info!("Sending a command to miniblock sealer: {:?}", command);

        self.commands_sender
            .send(command)
            .await
            .expect(Self::SHUTDOWN_MSG);

        let elapsed = start.elapsed();
        let queue_capacity = self.commands_sender.capacity();
        olaos_logs::info!(
            "Enqueued sealing command for miniblock #{miniblock_number} (took {elapsed:?}; \
             available queue capacity: {queue_capacity})"
        );

        if self.is_sync {
            self.wait_for_all_commands().await;
        }
    }

    #[olaos_logs::instrument(skip(self))]
    pub async fn wait_for_all_commands(&mut self) {
        olaos_logs::info!(
            "Requested waiting for miniblock seal queue to empty; current available capacity: {}",
            self.commands_sender.capacity()
        );

        let start = Instant::now();
        let completion_receiver = self.latest_completion_receiver.take();
        if let Some(completion_receiver) = completion_receiver {
            completion_receiver.await.expect(Self::SHUTDOWN_MSG);
        }

        let elapsed = start.elapsed();
        olaos_logs::info!("Miniblock seal queue is emptied (took {elapsed:?})");
    }
}

#[async_trait]
pub trait SequencerIO: 'static + Send {
    /// Returns the number of the currently processed L1 batch.
    fn current_l1_batch_number(&self) -> L1BatchNumber;
    /// Returns the number of the currently processed miniblock (aka L2 block).
    fn current_miniblock_number(&self) -> MiniblockNumber;
    /// Returns the data on the batch that was not sealed before the server restart.
    /// See `PendingBatchData` doc-comment for details.
    async fn load_pending_batch(&mut self) -> Option<PendingBatchData>;
    /// Blocks for up to `max_wait` until the parameters for the next L1 batch are available.
    /// Returns the data required to initialize the VM for the next batch.
    async fn wait_for_new_batch_params(&mut self, max_wait: Duration) -> Option<L1BatchParams>;
    /// Blocks for up to `max_wait` until the parameters for the next miniblock are available.
    /// Right now it's only a timestamp.
    async fn wait_for_new_miniblock_params(
        &mut self,
        max_wait: Duration,
        prev_miniblock_timestamp: u64,
    ) -> Option<u64>;
    /// Blocks for up to `max_wait` until the next transaction is available for execution.
    /// Returns `None` if no transaction became available until the timeout.
    async fn wait_for_next_tx(&mut self, max_wait: Duration) -> Option<Transaction>;
    /// Marks the transaction as "not executed", so it can be retrieved from the IO again.
    async fn rollback(&mut self, tx: Transaction);
    /// Marks the transaction as "rejected", e.g. one that is not correct and can't be executed.
    async fn reject(&mut self, tx: &Transaction, error: &str);
    /// Marks the miniblock (aka L2 block) as sealed.
    /// Returns the timestamp for the next miniblock.
    async fn seal_miniblock(&mut self, updates_manager: &UpdatesManager);
    /// Marks the L1 batch as sealed.
    async fn seal_l1_batch(
        &mut self,
        block_result: VmBlockResult,
        updates_manager: UpdatesManager,
        block_context: DerivedBlockContext,
    );
    /// Loads protocol version of the previous l1 batch.
    async fn load_previous_batch_version_id(&mut self) -> Option<ProtocolVersionId>;
    /// Loads protocol upgrade tx for given version.
    async fn load_upgrade_tx(&mut self, version_id: ProtocolVersionId)
        -> Option<ProtocolUpgradeTx>;
}

impl fmt::Debug for dyn SequencerIO {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SequencerIO")
            .field("current_l1_batch_number", &self.current_l1_batch_number())
            .field("current_miniblock_number", &self.current_miniblock_number())
            .finish()
    }
}

#[derive(Debug)]
pub struct PendingBatchData {
    /// Data used to initialize the pending batch. We have to make sure that all the parameters
    /// (e.g. timestamp) are the same, so transaction would have the same result after re-execution.
    pub(crate) params: L1BatchParams,
    /// List of miniblocks and corresponding transactions that were executed within batch.
    pub(crate) pending_miniblocks: Vec<MiniblockReexecuteData>,
}

#[derive(Debug, Clone)]
pub struct L1BatchParams {
    pub context_mode: BlockContextMode,
    pub properties: BlockProperties,
    pub base_system_contracts: BaseSystemContracts,
    pub protocol_version: ProtocolVersionId,
}

impl L1BatchParams {
    pub fn block_number(&self) -> u32 {
        self.context_mode.inner_block_context().context.block_number
    }
}
