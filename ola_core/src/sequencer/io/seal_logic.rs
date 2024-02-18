use itertools::Itertools;
use ola_config::constants::contracts::ACCOUNT_CODE_STORAGE_ADDRESS;
use ola_vm::{
    vm::VmBlockResult,
    vm_with_bootloader::{BlockContextMode, DerivedBlockContext},
};

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use ola_dal::StorageProcessor;
use ola_types::{
    block::{L1BatchHeader, MiniblockHeader},
    events::VmEvent,
    log::{LogQuery, StorageLog, StorageLogQuery},
    tx::{IncludedTxLocation, TransactionExecutionResult},
    AccountTreeId, Address, L1BatchNumber, MiniblockNumber, StorageKey, StorageValue, Transaction,
    H256, U256,
};
use ola_utils::{misc::miniblock_hash, u256_to_h256};

use crate::sequencer::{
    extractors,
    io::{
        common::set_missing_initial_writes_indices,
        sort_storage_access::sort_storage_access_queries,
    },
    updates::{l1_batch_updates::L1BatchUpdates, MiniblockSealCommand, UpdatesManager},
};

#[derive(Debug, Clone, Copy)]
struct SealProgressMetricNames {
    target: &'static str,
}

impl SealProgressMetricNames {
    const L1_BATCH: Self = Self { target: "L1 batch" };

    const MINIBLOCK: Self = Self {
        target: "miniblock",
    };
}

#[derive(Debug)]
struct SealProgress {
    metric_names: SealProgressMetricNames,
    stage_start: Instant,
}

impl SealProgress {
    fn for_l1_batch() -> Self {
        Self {
            metric_names: SealProgressMetricNames::L1_BATCH,
            stage_start: Instant::now(),
        }
    }

    fn for_miniblock() -> Self {
        Self {
            metric_names: SealProgressMetricNames::MINIBLOCK,
            stage_start: Instant::now(),
        }
    }

    fn end_stage(&mut self, stage: &'static str, count: Option<usize>) {
        const MIN_STAGE_DURATION_TO_REPORT: Duration = Duration::from_millis(10);

        let elapsed = self.stage_start.elapsed();
        if elapsed > MIN_STAGE_DURATION_TO_REPORT {
            let target = self.metric_names.target;
            olaos_logs::info!(
                "{target} execution stage {stage} took {elapsed:?} with count {count:?}"
            );
        }

        self.stage_start = Instant::now();
    }
}

impl MiniblockSealCommand {
    pub async fn seal(&self, storage: &mut StorageProcessor<'_>) {
        self.seal_inner(storage, false).await;
    }

    async fn seal_inner(&self, storage: &mut StorageProcessor<'_>, is_fictive: bool) {
        self.assert_valid_miniblock(is_fictive);

        let l1_batch_number = self.l1_batch_number;
        let miniblock_number = self.miniblock_number;
        let started_at = Instant::now();
        let mut progress = SealProgress::for_miniblock();

        let (l1_tx_count, l2_tx_count) = l1_l2_tx_count(&self.miniblock.executed_transactions);
        let (writes_count, reads_count) =
            storage_log_query_write_read_counts(&self.miniblock.storage_logs);
        olaos_logs::info!(
            "Sealing miniblock {miniblock_number} (L1 batch {l1_batch_number}) \
             with {total_tx_count} ({l2_tx_count} L2 + {l1_tx_count} L1) txs, \
             {event_count} events, {reads_count} reads, {writes_count} writes",
            total_tx_count = l1_tx_count + l2_tx_count,
            event_count = self.miniblock.events.len()
        );

        let mut transaction = storage.start_transaction().await;
        let miniblock_header = MiniblockHeader {
            number: miniblock_number,
            timestamp: self.miniblock.timestamp,
            hash: miniblock_hash(miniblock_number),
            l1_tx_count: l1_tx_count as u16,
            l2_tx_count: l2_tx_count as u16,
            base_system_contracts_hashes: self.base_system_contracts_hashes,
            protocol_version: Some(self.protocol_version),
        };

        transaction
            .blocks_dal()
            .insert_miniblock(&miniblock_header)
            .await;
        progress.end_stage("insert_miniblock_header", None);

        transaction
            .transactions_dal()
            .mark_txs_as_executed_in_miniblock(
                miniblock_number,
                &self.miniblock.executed_transactions,
            )
            .await;
        progress.end_stage(
            "mark_transactions_in_miniblock",
            Some(self.miniblock.executed_transactions.len()),
        );

        let write_logs = self.extract_write_logs(is_fictive);
        // let write_log_count = write_logs.iter().map(|(_, logs)| logs.len()).sum();

        transaction
            .storage_logs_dal()
            .insert_storage_logs(miniblock_number, &write_logs)
            .await;
        // progress.end_stage("insert_storage_logs", Some(write_log_count));

        let unique_updates = transaction
            .storage_dal()
            .apply_storage_logs(&write_logs)
            .await;
        // progress.end_stage("apply_storage_logs", Some(write_log_count));

        let new_factory_deps = &self.miniblock.new_factory_deps;
        let new_factory_deps_count = new_factory_deps.len();
        if !new_factory_deps.is_empty() {
            transaction
                .storage_dal()
                .insert_factory_deps(miniblock_number, new_factory_deps)
                .await;
        }
        progress.end_stage("insert_factory_deps", Some(new_factory_deps_count));

        // Factory deps should be inserted before using `count_deployed_contracts`.
        let deployed_contract_count = Self::count_deployed_contracts(&unique_updates);
        progress.end_stage("extract_contracts_deployed", Some(deployed_contract_count));

        // TODO: do not process L1 & L2 bridge & tokens
        // let added_tokens = extract_added_tokens(self.l2_erc20_bridge_addr, &self.miniblock.events);
        // progress.end_stage("extract_added_tokens", Some(added_tokens.len()));
        // let added_tokens_len = added_tokens.len();
        // if !added_tokens.is_empty() {
        //     transaction.tokens_dal().add_tokens(added_tokens).await;
        // }
        // progress.end_stage("insert_tokens", Some(added_tokens_len));

        let miniblock_events = self.extract_events(is_fictive);
        let miniblock_event_count = miniblock_events
            .iter()
            .map(|(_, events)| events.len())
            .sum();
        progress.end_stage("extract_events", Some(miniblock_event_count));
        transaction
            .events_dal()
            .save_events(miniblock_number, &miniblock_events)
            .await;
        progress.end_stage("insert_events", Some(miniblock_event_count));

        transaction.commit().await;
        progress.end_stage("commit_miniblock", None);
    }

    fn assert_valid_miniblock(&self, is_fictive: bool) {
        assert_eq!(self.miniblock.executed_transactions.is_empty(), is_fictive);

        let first_tx_index = self.first_tx_index;
        let next_tx_index = first_tx_index + self.miniblock.executed_transactions.len();
        let tx_index_range = if is_fictive {
            next_tx_index..(next_tx_index + 1)
        } else {
            first_tx_index..next_tx_index
        };

        for event in &self.miniblock.events {
            let tx_index = event.location.1 as usize;
            assert!(tx_index_range.contains(&tx_index));
        }
        for storage_log in &self.miniblock.storage_logs {
            let tx_index = storage_log.log_query.tx_number_in_block as usize;
            assert!(tx_index_range.contains(&tx_index));
        }
    }

    fn extract_write_logs(&self, is_fictive: bool) -> Vec<(H256, Vec<StorageLog>)> {
        let logs = self.miniblock.storage_logs.iter();
        let grouped_logs = logs.group_by(|log| log.log_query.tx_number_in_block);

        let grouped_logs = grouped_logs.into_iter().map(|(tx_index, logs)| {
            let tx_hash = if is_fictive {
                assert_eq!(tx_index as usize, self.first_tx_index);
                H256::zero()
            } else {
                self.transaction(tx_index as usize).hash()
            };
            let logs = logs
                .filter(|&log| log.log_query.rw_flag)
                .map(StorageLog::from_log_query);
            (tx_hash, logs.collect())
        });
        grouped_logs.collect()
    }

    fn transaction(&self, index: usize) -> &Transaction {
        let tx_result = &self.miniblock.executed_transactions[index - self.first_tx_index];
        &tx_result.transaction
    }

    fn count_deployed_contracts(
        unique_updates: &HashMap<StorageKey, (H256, StorageValue)>,
    ) -> usize {
        let mut count = 0;
        for (key, (_, value)) in unique_updates {
            if *key.account().address() == ACCOUNT_CODE_STORAGE_ADDRESS {
                let bytecode_hash = *value;
                //  For now, we expected that if the `bytecode_hash` is zero, the contract was not deployed
                //  in the first place, so we don't do anything
                if bytecode_hash != H256::zero() {
                    count += 1;
                }
            }
        }
        count
    }

    fn extract_events(&self, is_fictive: bool) -> Vec<(IncludedTxLocation, Vec<&VmEvent>)> {
        self.group_by_tx_location(&self.miniblock.events, is_fictive, |event| event.location.1)
    }

    fn group_by_tx_location<'a, T>(
        &'a self,
        entries: &'a [T],
        is_fictive: bool,
        tx_location: impl Fn(&T) -> u32,
    ) -> Vec<(IncludedTxLocation, Vec<&'a T>)> {
        let grouped_entries = entries.iter().group_by(|&entry| tx_location(entry));
        let grouped_entries = grouped_entries.into_iter().map(|(tx_index, entries)| {
            let (tx_hash, tx_initiator_address) = if is_fictive {
                assert_eq!(tx_index as usize, self.first_tx_index);
                (H256::zero(), Address::zero())
            } else {
                let tx = self.transaction(tx_index as usize);
                (tx.hash(), tx.initiator_account())
            };

            let location = IncludedTxLocation {
                tx_hash,
                tx_index_in_miniblock: tx_index - self.first_tx_index as u32,
                tx_initiator_address,
            };
            (location, entries.collect())
        });
        grouped_entries.collect()
    }
}

impl UpdatesManager {
    /// Persists an L1 batch in the storage.
    /// This action includes a creation of an empty "fictive" miniblock that contains
    /// the events generated during the bootloader "tip phase".
    #[olaos_logs::instrument(
        skip_all,
        fields(current_miniblock_number, current_l1_batch_number, block_context)
    )]
    pub(crate) async fn seal_l1_batch(
        mut self,
        storage: &mut StorageProcessor<'_>,
        current_miniblock_number: MiniblockNumber,
        current_l1_batch_number: L1BatchNumber,
        block_result: VmBlockResult,
        block_context: DerivedBlockContext,
    ) {
        let mut progress = SealProgress::for_l1_batch();
        let mut transaction = storage.start_transaction().await;

        // The vm execution was paused right after the last transaction was executed.
        // There is some post-processing work that the VM needs to do before the block is fully processed.
        let VmBlockResult {
            full_result,
            block_tip_result,
        } = block_result;
        assert!(
            full_result.revert_reason.is_none(),
            "VM must not revert when finalizing block. Revert reason: {:?}",
            full_result.revert_reason
        );
        progress.end_stage("vm_finalization", None);

        self.extend_from_fictive_transaction(block_tip_result.logs);
        // Seal fictive miniblock with last events and storage logs.
        let miniblock_command =
            self.seal_miniblock_command(current_l1_batch_number, current_miniblock_number);
        miniblock_command.seal_inner(&mut transaction, true).await;
        progress.end_stage("fictive_miniblock", None);

        let (_, deduped_log_queries) = sort_storage_access_queries(
            full_result
                .storage_log_queries
                .iter()
                .map(|log| &log.log_query),
        );
        progress.end_stage("log_deduplication", Some(0));

        let (l1_tx_count, l2_tx_count) = l1_l2_tx_count(&self.l1_batch.executed_transactions);
        let (writes_count, reads_count) =
            storage_log_query_write_read_counts(&full_result.storage_log_queries);
        let (dedup_writes_count, dedup_reads_count) =
            log_query_write_read_counts(deduped_log_queries.iter());

        olaos_logs::info!(
            "Sealing L1 batch {current_l1_batch_number} with {total_tx_count} \
                ({l2_tx_count} L2 + {l1_tx_count} L1) txs, \
                {event_count} events, {reads_count} reads ({dedup_reads_count} deduped), \
                {writes_count} writes ({dedup_writes_count} deduped)",
            total_tx_count = l1_tx_count + l2_tx_count,
            event_count = full_result.events.len()
        );

        let (prev_hash, prev_timestamp) =
            extractors::wait_for_prev_l1_batch_params(&mut transaction, current_l1_batch_number)
                .await;
        let timestamp = block_context.context.block_timestamp;
        assert!(
            prev_timestamp < timestamp,
            "Cannot seal L1 batch #{}: Timestamp of previous L1 batch ({}) >= provisional L1 batch timestamp ({}), \
             meaning that L1 batch will be rejected by the bootloader",
            current_l1_batch_number,
            extractors::display_timestamp(prev_timestamp),
            extractors::display_timestamp(timestamp)
        );

        let l1_batch = L1BatchHeader {
            number: current_l1_batch_number,
            is_finished: true,
            timestamp,
            l1_tx_count: l1_tx_count as u16,
            l2_tx_count: l2_tx_count as u16,
            used_contract_hashes: vec![],
            base_system_contracts_hashes: self.base_system_contract_hashes(),
            protocol_version: Some(self.protocol_version()),
        };

        let block_context_properties = BlockContextMode::NewBlock(block_context, prev_hash);
        let initial_bootloader_contents =
            Self::initial_bootloader_memory(&self.l1_batch, block_context_properties);

        transaction
            .blocks_dal()
            .insert_l1_batch(&l1_batch, &initial_bootloader_contents)
            .await;
        progress.end_stage("insert_l1_batch_header", None);

        transaction
            .blocks_dal()
            .mark_miniblocks_as_executed_in_l1_batch(current_l1_batch_number)
            .await;
        progress.end_stage("set_l1_batch_number_for_miniblocks", None);

        transaction
            .transactions_dal()
            .mark_txs_as_executed_in_l1_batch(
                current_l1_batch_number,
                &self.l1_batch.executed_transactions,
            )
            .await;
        progress.end_stage("mark_txs_as_executed_in_l1_batch", None);

        let (deduplicated_writes, protective_reads): (Vec<_>, Vec<_>) = deduped_log_queries
            .into_iter()
            .partition(|log_query| log_query.rw_flag);
        transaction
            .storage_logs_dedup_dal()
            .insert_protective_reads(current_l1_batch_number, &protective_reads)
            .await;
        progress.end_stage("insert_protective_reads", Some(protective_reads.len()));

        let deduplicated_writes_hashed_keys: Vec<_> = deduplicated_writes
            .iter()
            .map(|log| {
                H256(StorageKey::raw_hashed_key(
                    &log.address,
                    &u256_to_h256(log.key),
                ))
            })
            .collect();
        let non_initial_writes = transaction
            .storage_logs_dedup_dal()
            .filter_written_slots(&deduplicated_writes_hashed_keys)
            .await;
        progress.end_stage("filter_written_slots", Some(deduplicated_writes.len()));

        let written_storage_keys: Vec<_> = deduplicated_writes
            .iter()
            .filter_map(|log| {
                let key = StorageKey::new(AccountTreeId::new(log.address), u256_to_h256(log.key));
                (!non_initial_writes.contains(&key.hashed_key())).then_some(key)
            })
            .collect();

        // One-time migration completion for initial writes' indices.
        set_missing_initial_writes_indices(&mut transaction).await;
        progress.end_stage("set_missing_initial_writes_indices", None);

        transaction
            .storage_logs_dedup_dal()
            .insert_initial_writes(current_l1_batch_number, &written_storage_keys)
            .await;
        progress.end_stage("insert_initial_writes", Some(written_storage_keys.len()));

        transaction.commit().await;
        progress.end_stage("commit_l1_batch", None);

        let writes_metrics = self.storage_writes_deduplicator.metrics();
        // Sanity check metrics.
        assert_eq!(
            deduplicated_writes.len(),
            writes_metrics.initial_storage_writes + writes_metrics.repeated_storage_writes,
            "Results of in-flight and common deduplications are mismatched"
        );
    }

    pub(crate) fn initial_bootloader_memory(
        _updates_accumulator: &L1BatchUpdates,
        _block_context: BlockContextMode,
    ) -> Vec<(usize, U256)> {
        // TODO: @Pierre return tape
        vec![(0, U256::default())]
        // let transactions_data = updates_accumulator
        //     .executed_transactions
        //     .iter()
        //     .map(|res| res.transaction.clone().into())
        //     .collect();

        // let refunds = updates_accumulator
        //     .executed_transactions
        //     .iter()
        //     .map(|res| res.operator_suggested_refund)
        //     .collect();

        // let compressed_bytecodes = updates_accumulator
        //     .executed_transactions
        //     .iter()
        //     .map(|res| res.compressed_bytecodes.clone())
        //     .collect();

        // get_bootloader_memory(
        //     transactions_data,
        //     refunds,
        //     compressed_bytecodes,
        //     TxExecutionMode::VerifyExecute,
        //     block_context,
        // )
    }
}

fn l1_l2_tx_count(executed_transactions: &[TransactionExecutionResult]) -> (usize, usize) {
    let l1_tx_count = 0;
    let mut l2_tx_count = 0;

    for _tx in executed_transactions {
        // TODO: add l1_tx_count
        l2_tx_count += 1;
    }
    (l1_tx_count, l2_tx_count)
}

fn log_query_write_read_counts<'a>(logs: impl Iterator<Item = &'a LogQuery>) -> (usize, usize) {
    let mut reads_count = 0;
    let mut writes_count = 0;

    for log in logs {
        if log.rw_flag {
            writes_count += 1;
        } else {
            reads_count += 1;
        }
    }
    (writes_count, reads_count)
}

fn storage_log_query_write_read_counts(logs: &[StorageLogQuery]) -> (usize, usize) {
    log_query_write_read_counts(logs.iter().map(|log| &log.log_query))
}
