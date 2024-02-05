use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use ola_config::{database::load_db_config, sequencer::load_network_config};
use ola_dal::{connection::ConnectionPool, SqlxError, StorageProcessor};
use ola_state::{postgres::PostgresStorage, storage_view::StorageView, ReadStorage, WriteStorage};
use ola_types::{
    api::{BlockId, BlockNumber},
    get_nonce_key,
    utils::decompose_full_nonce,
    ExecuteTransactionCommon, L1BatchNumber, MiniblockNumber, Nonce, StorageKey, Transaction, H256,
    U256,
};
use ola_utils::{h256_to_u256, time::seconds_since_epoch, u256_to_h256};
use ola_vm::vm_with_bootloader::{
    BlockContext, BlockContextMode, BlockProperties, DerivedBlockContext,
};
use olavm_core::state::error::StateError;
use zk_vm::{BlockInfo, PreExecutor, TxInfo};

use super::{execute::TxExecutionArgs, BlockArgs, TxSharedArgs, VmPermit};

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_vm_in_sandbox2<T>(
    vm_permit: VmPermit,
    shared_args: TxSharedArgs,
    execution_args: &TxExecutionArgs,
    connection_pool: &ConnectionPool,
    tx: Transaction,
    block_args: BlockArgs,
    storage_read_cache: HashMap<StorageKey, H256>,
    // apply: impl FnOnce(&mut Box<VmInstance<'_, HistoryDisabled>>, Transaction) -> T,
) -> (T, HashMap<StorageKey, H256>) {
    let stage_started_at = Instant::now();
    let span = tracing::debug_span!("initialization").entered();

    let rt_handle = vm_permit.rt_handle();
    let mut connection = rt_handle.block_on(connection_pool.access_storage_tagged("api"));
    let connection_acquire_time = stage_started_at.elapsed();
    // We don't want to emit too many logs.
    if connection_acquire_time > Duration::from_millis(10) {
        olaos_logs::info!(
            "Obtained connection (took {:?})",
            stage_started_at.elapsed()
        );
    }

    let resolve_started_at = Instant::now();
    let (state_block_number, vm_block_number) = rt_handle
        .block_on(block_args.resolve_block_numbers(&mut connection))
        .expect("Failed resolving block numbers");
    let resolve_time = resolve_started_at.elapsed();
    // We don't want to emit too many logs.
    if resolve_time > Duration::from_millis(10) {
        olaos_logs::info!(
            "Resolved block numbers (took {:?})",
            resolve_started_at.elapsed()
        );
    }

    if block_args.resolves_to_latest_sealed_miniblock() {
        shared_args
            .caches
            .schedule_values_update(state_block_number);
    }
    let block_timestamp = block_args.block_timestamp_seconds();

    let storage = PostgresStorage::new(rt_handle.clone(), connection, state_block_number, false)
        .with_caches(shared_args.caches);
    // Moving `storage_read_cache` to `storage_view`. It will be moved back once execution is finished and `storage_view` is not needed.
    let mut storage_view = StorageView::new_with_read_keys(storage, storage_read_cache);

    let storage_view_setup_started_at = Instant::now();
    if let Some(nonce) = execution_args.enforced_nonce {
        let nonce_key = get_nonce_key(&tx.initiator_account());
        let nonce_value = u256_to_h256(U256::from(nonce.0));
        storage_view.set_value(nonce_key, nonce_value);
    }

    let payer = tx.payer();
    let storage_view_setup_time = storage_view_setup_started_at.elapsed();
    // We don't want to emit too many logs.
    if storage_view_setup_time > Duration::from_millis(10) {
        olaos_logs::info!("Prepared the storage view (took {storage_view_setup_time:?})",);
    }

    // let mut oracle_tools = vm::OracleTools::new(&mut storage_view, HistoryDisabled);
    let block_properties = BlockProperties {
        default_aa_code_hash: h256_to_u256(shared_args.base_system_contracts.default_aa.hash),
    };

    let block_context = DerivedBlockContext {
        context: BlockContext {
            block_number: vm_block_number.0,
            block_timestamp,
            operator_address: *shared_args.operator_account.address(),
        },
    };

    // Since this method assumes that the block vm_block_number-1 is present in the DB, it means that its hash
    // has already been stored in the VM.
    let block_context_properties = BlockContextMode::OverrideCurrent(block_context);

    let mut vm = init_vm(
        &mut oracle_tools,
        block_context_properties,
        &block_properties,
        execution_args.execution_mode,
        &shared_args.base_system_contracts,
    );

    span.exit();

    let tx_id = format!(
        "{:?}-{}",
        tx.initiator_account(),
        tx.nonce().unwrap_or(Nonce(0))
    );
    let stage_started_at = Instant::now();
    let result = apply(&mut vm, tx);
    let vm_execution_took = stage_started_at.elapsed();

    drop(vm_permit); // Ensure that the permit lives until this point

    // Move `read_storage_keys` from `storage_view` back to cache.
    (result, storage_view.into_read_storage_keys())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_vm_in_sandbox(
    vm_permit: VmPermit,
    shared_args: TxSharedArgs,
    connection_pool: &ConnectionPool,
    tx: Transaction,
    block_args: BlockArgs,
) -> Result<(), StateError> {
    let rt_handle = vm_permit.rt_handle();
    let mut connection = rt_handle.block_on(connection_pool.access_storage_tagged("api"));

    let (state_block_number, vm_block_number) = rt_handle
        .block_on(block_args.resolve_block_numbers(&mut connection))
        .expect("Failed resolving block numbers");

    if block_args.resolves_to_latest_sealed_miniblock() {
        shared_args
            .caches
            .schedule_values_update(state_block_number);
    }
    let block_timestamp = block_args.block_timestamp_seconds();

    let db_config = load_db_config().expect("failed to load database config");
    let network = load_network_config().expect("failed to load network config");

    let hash = tx.hash();
    let tx_info = match tx.common_data {
        ExecuteTransactionCommon::L2(common_data) => {
            let version = common_data.transaction_type as u32;
            let caller_address = common_data.initiator_address.to_fixed_bytes();
            let nonce = common_data.nonce.0;

            let to_u8_32 = |v: &Vec<u8>| {
                let mut array = [0; 32];
                array.copy_from_slice(&v[..32]);
                array
            };

            let r = common_data.signature[0..32].to_vec();
            let s = common_data.signature[32..64].to_vec();

            TxInfo {
                version,
                caller_address,
                calldata: tx.execute.calldata.to_vec(),
                nonce,
                signature_r: to_u8_32(&r),
                signature_s: to_u8_32(&s),
                tx_hash: hash.to_fixed_bytes(),
            }
        }
        ExecuteTransactionCommon::ProtocolUpgrade(_) => panic!("ProtocolUpgrade not supported"),
    };

    let block_info = BlockInfo {
        block_number: vm_block_number.0,
        block_timestamp,
        sequencer_address: shared_args.operator_account.address().to_fixed_bytes(),
        chain_id: network.ola_network_id,
    };

    let executor = PreExecutor::new(
        block_info,
        db_config.merkle_tree.path,
        db_config.sequencer_db_path,
    );

    drop(vm_permit);

    executor.execute(tx_info)
}

impl BlockArgs {
    fn is_pending_miniblock(&self) -> bool {
        matches!(self.block_id, BlockId::Number(BlockNumber::Pending))
    }

    fn resolves_to_latest_sealed_miniblock(&self) -> bool {
        matches!(
            self.block_id,
            BlockId::Number(BlockNumber::Pending | BlockNumber::Latest | BlockNumber::Committed)
        )
    }

    async fn resolve_block_numbers(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> Result<(MiniblockNumber, L1BatchNumber), SqlxError> {
        Ok(if self.is_pending_miniblock() {
            let sealed_l1_batch_number = connection
                .blocks_web3_dal()
                .get_sealed_l1_batch_number()
                .await?;
            let sealed_miniblock_number = connection
                .blocks_web3_dal()
                .get_sealed_miniblock_number()
                .await?;
            (sealed_miniblock_number, sealed_l1_batch_number + 1)
        } else {
            let l1_batch_number = connection
                .storage_web3_dal()
                .resolve_l1_batch_number_of_miniblock(self.resolved_block_number)
                .await?
                .expected_l1_batch();
            (self.resolved_block_number, l1_batch_number)
        })
    }

    fn block_timestamp_seconds(&self) -> u64 {
        if self.is_pending_miniblock() {
            seconds_since_epoch()
        } else {
            self.block_timestamp_s.unwrap_or_else(|| {
                panic!(
                    "Block timestamp is `None`, `block_id`: {:?}, `resolved_block_number`: {}",
                    self.block_id, self.resolved_block_number.0
                );
            })
        }
    }
}
