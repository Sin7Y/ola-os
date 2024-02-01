use async_trait::async_trait;
use ola_utils::time::millis_since_epoch;
use ola_vm::{vm::VmBlockResult, vm_with_bootloader::DerivedBlockContext};

use std::{
    cmp,
    collections::HashMap,
    time::{Duration, Instant},
};

use ola_config::sequencer::SequencerConfig;
use ola_dal::connection::ConnectionPool;
use ola_types::{
    protocol_version::{ProtocolUpgradeTx, ProtocolVersionId},
    Address, L1BatchNumber, MiniblockNumber, Transaction, U256,
};

use crate::sequencer::{extractors, types::MempoolGuard, updates::UpdatesManager};

use super::{
    common::{l1_batch_params, load_pending_batch, poll_iters},
    L1BatchParams, MiniblockSealerHandle, PendingBatchData, SequencerIO,
};

#[derive(Debug)]
pub(crate) struct MempoolIO {
    mempool: MempoolGuard,
    pool: ConnectionPool,
    current_miniblock_number: MiniblockNumber,
    miniblock_sealer_handle: MiniblockSealerHandle,
    current_l1_batch_number: L1BatchNumber,
    fee_account: Address,
    delay_interval: Duration,
}

impl MempoolIO {
    pub(in crate::sequencer) async fn new(
        mempool: MempoolGuard,
        miniblock_sealer_handle: MiniblockSealerHandle,
        pool: ConnectionPool,
        config: &SequencerConfig,
        delay_interval: Duration,
    ) -> Self {
        let mut storage = pool.access_storage_tagged("sequencer").await;
        let last_sealed_l1_batch_header = storage.blocks_dal().get_newest_l1_batch_header().await;
        let last_miniblock_number = storage.blocks_dal().get_sealed_miniblock_number().await;
        drop(storage);

        Self {
            mempool,
            pool,
            current_l1_batch_number: last_sealed_l1_batch_header.number + 1,
            miniblock_sealer_handle,
            current_miniblock_number: last_miniblock_number + 1,
            fee_account: config.fee_account_addr,
            delay_interval,
        }
    }
}

#[async_trait]
impl SequencerIO for MempoolIO {
    fn current_l1_batch_number(&self) -> L1BatchNumber {
        self.current_l1_batch_number
    }

    fn current_miniblock_number(&self) -> MiniblockNumber {
        self.current_miniblock_number
    }

    #[olaos_logs::instrument(skip_all)]
    async fn load_pending_batch(&mut self) -> Option<PendingBatchData> {
        let mut storage = self.pool.access_storage_tagged("sequencer").await;

        let PendingBatchData {
            params,
            pending_miniblocks,
        } = load_pending_batch(&mut storage, self.current_l1_batch_number, self.fee_account)
            .await?;

        Some(PendingBatchData {
            params,
            pending_miniblocks,
        })
    }

    #[olaos_logs::instrument(skip_all)]
    async fn wait_for_new_batch_params(&mut self, max_wait: Duration) -> Option<L1BatchParams> {
        let deadline = Instant::now() + max_wait;

        olaos_logs::info!(
            "start wait_for_new_batch_params with deadline {:?}",
            deadline
        );

        // Block until at least one transaction in the mempool can match the filter (or timeout happens).
        // This is needed to ensure that block timestamp is not too old.
        for _ in 0..poll_iters(self.delay_interval, max_wait) {
            // We only need to get the root hash when we're certain that we have a new transaction.
            if !self.mempool.has_next() {
                tokio::time::sleep(self.delay_interval).await;
                continue;
            }

            olaos_logs::info!("mempool has a new tx");

            let prev_l1_batch_hash = self.load_previous_l1_batch_hash().await;
            let prev_miniblock_timestamp = self.load_previous_miniblock_timestamp().await;
            // We cannot create two L1 batches or miniblocks with the same timestamp (forbidden by the bootloader).
            // Hence, we wait until the current timestamp is larger than the timestamp of the previous miniblock.
            // We can use `timeout_at` since `sleep_past` is cancel-safe; it only uses `sleep()` async calls.
            let current_timestamp = tokio::time::timeout_at(
                deadline.into(),
                sleep_past(prev_miniblock_timestamp, self.current_miniblock_number),
            );
            let current_timestamp = current_timestamp.await.ok()?;

            let mut storage = self.pool.access_storage().await;
            let (base_system_contracts, protocol_version) = storage
                .protocol_versions_dal()
                .base_system_contracts_by_timestamp(current_timestamp as i64)
                .await;

            let l1_batch_params = l1_batch_params(
                self.current_l1_batch_number,
                self.fee_account,
                current_timestamp,
                prev_l1_batch_hash,
                base_system_contracts,
                protocol_version,
            );

            olaos_logs::info!("get new l1_batch_params {:?}", l1_batch_params);

            return Some(l1_batch_params);
        }
        None
    }

    #[olaos_logs::instrument(skip(self))]
    async fn wait_for_new_miniblock_params(
        &mut self,
        max_wait: Duration,
        prev_miniblock_timestamp: u64,
    ) -> Option<u64> {
        // We must provide different timestamps for each miniblock.
        // If miniblock sealing interval is greater than 1 second then `sleep_past` won't actually sleep.
        let current_timestamp = tokio::time::timeout(
            max_wait,
            sleep_past(prev_miniblock_timestamp, self.current_miniblock_number),
        );
        current_timestamp.await.ok()
    }

    #[olaos_logs::instrument(skip_all)]
    async fn wait_for_next_tx(&mut self, max_wait: Duration) -> Option<Transaction> {
        for _ in 0..poll_iters(self.delay_interval, max_wait) {
            let res = self.mempool.next_transaction();
            if let Some(res) = res {
                return Some(res);
            } else {
                tokio::time::sleep(self.delay_interval).await;
                continue;
            }
        }
        None
    }

    #[olaos_logs::instrument(skip(self))]
    async fn rollback(&mut self, tx: Transaction) {
        // Reset nonces in the mempool.
        self.mempool.rollback(&tx);
        // Insert the transaction back.
        self.mempool.insert(vec![tx], HashMap::new());
    }

    #[olaos_logs::instrument(skip(self))]
    async fn reject(&mut self, rejected: &Transaction, error: &str) {
        // TODO: uncomment when add L1 transaction
        // assert!(
        //     !rejected.is_l1(),
        //     "L1 transactions should not be rejected: {}",
        //     error
        // );

        // Reset the nonces in the mempool, but don't insert the transaction back.
        self.mempool.rollback(rejected);

        // Mark tx as rejected in the storage.
        let mut storage = self.pool.access_storage_tagged("sequencer").await;
        olaos_logs::warn!(
            "transaction {} is rejected with error {}",
            rejected.hash(),
            error
        );
        storage
            .transactions_dal()
            .mark_tx_as_rejected(rejected.hash(), &format!("rejected: {}", error))
            .await;
    }

    #[olaos_logs::instrument(skip_all)]
    async fn seal_miniblock(&mut self, updates_manager: &UpdatesManager) {
        let command = updates_manager
            .seal_miniblock_command(self.current_l1_batch_number, self.current_miniblock_number);
        self.miniblock_sealer_handle.submit(command).await;
        self.current_miniblock_number += 1;
    }

    #[olaos_logs::instrument(skip_all, fields(block_context))]
    async fn seal_l1_batch(
        &mut self,
        block_result: VmBlockResult,
        updates_manager: UpdatesManager,
        block_context: DerivedBlockContext,
    ) {
        assert_eq!(
            updates_manager.batch_timestamp(),
            block_context.context.block_timestamp,
            "Batch timestamps don't match, batch number {}",
            self.current_l1_batch_number()
        );

        // We cannot start sealing an L1 batch until we've sealed all miniblocks included in it.
        self.miniblock_sealer_handle.wait_for_all_commands().await;

        let pool = self.pool.clone();
        let mut storage = pool.access_storage_tagged("sequencer").await;
        updates_manager
            .seal_l1_batch(
                &mut storage,
                self.current_miniblock_number,
                self.current_l1_batch_number,
                block_result,
                block_context,
            )
            .await;
        self.current_miniblock_number += 1; // Due to fictive miniblock being sealed.
        self.current_l1_batch_number += 1;
    }

    #[olaos_logs::instrument(skip_all)]
    async fn load_previous_batch_version_id(&mut self) -> Option<ProtocolVersionId> {
        let mut storage = self.pool.access_storage().await;
        storage
            .blocks_dal()
            .get_batch_protocol_version_id(self.current_l1_batch_number - 1)
            .await
    }

    #[olaos_logs::instrument(skip_all)]
    async fn load_upgrade_tx(
        &mut self,
        version_id: ProtocolVersionId,
    ) -> Option<ProtocolUpgradeTx> {
        let mut storage = self.pool.access_storage().await;
        storage
            .protocol_versions_dal()
            .get_protocol_upgrade_tx(version_id)
            .await
    }
}

impl MempoolIO {
    #[olaos_logs::instrument(skip_all)]
    async fn load_previous_l1_batch_hash(&self) -> U256 {
        olaos_logs::info!(
            "Getting previous L1 batch hash for L1 batch #{}",
            self.current_l1_batch_number
        );
        let _stage_started_at: Instant = Instant::now();

        let mut storage = self.pool.access_storage_tagged("sequencer").await;
        let (batch_hash, _) =
            extractors::wait_for_prev_l1_batch_params(&mut storage, self.current_l1_batch_number)
                .await;

        olaos_logs::info!(
            "Got previous L1 batch hash: {batch_hash:0>64x} for L1 batch #{}",
            self.current_l1_batch_number
        );
        batch_hash
    }

    async fn load_previous_miniblock_timestamp(&self) -> u64 {
        let mut storage = self.pool.access_storage_tagged("sequencer").await;

        storage
            .blocks_dal()
            .get_miniblock_timestamp(self.current_miniblock_number - 1)
            .await
            .expect("Previous miniblock must be sealed and header saved to DB")
    }
}

async fn sleep_past(timestamp: u64, miniblock: MiniblockNumber) -> u64 {
    let mut current_timestamp_millis = millis_since_epoch();
    let mut current_timestamp = (current_timestamp_millis / 1_000) as u64;
    match timestamp.cmp(&current_timestamp) {
        cmp::Ordering::Less => return current_timestamp,
        cmp::Ordering::Equal => {
            olaos_logs::info!(
                "Current timestamp {} for miniblock #{miniblock} is equal to previous miniblock timestamp; waiting until \
                 timestamp increases",
                extractors::display_timestamp(current_timestamp)
            );
        }
        cmp::Ordering::Greater => {
            // This situation can be triggered if the system keeper is started on a pod with a different
            // system time, or if it is buggy. Thus, a one-time error could require no actions if L1 batches
            // are expected to be generated frequently.
            olaos_logs::error!(
                "Previous miniblock timestamp {} is larger than the current timestamp {} for miniblock #{miniblock}",
                extractors::display_timestamp(timestamp),
                extractors::display_timestamp(current_timestamp)
            );
        }
    }

    // This loop should normally run once, since `tokio::time::sleep` sleeps *at least* the specified duration.
    // The logic is organized in a loop for marginal cases, such as the system time getting changed during `sleep()`.
    loop {
        // Time to catch up to `timestamp`; panic / underflow on subtraction is never triggered
        // since we've ensured that `timestamp >= current_timestamp`.
        let wait_seconds = timestamp - current_timestamp;
        // Time to wait until the current timestamp increases.
        let wait_millis = 1_001 - (current_timestamp_millis % 1_000) as u64;
        let wait = Duration::from_millis(wait_millis + wait_seconds * 1_000);

        tokio::time::sleep(wait).await;
        current_timestamp_millis = millis_since_epoch();
        current_timestamp = (current_timestamp_millis / 1_000) as u64;

        if current_timestamp > timestamp {
            return current_timestamp;
        }
    }
}
