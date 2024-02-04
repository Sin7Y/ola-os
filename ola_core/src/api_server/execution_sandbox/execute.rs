use std::collections::HashMap;

use ola_dal::connection::ConnectionPool;
use ola_types::{l2::L2Tx, Transaction};
use ola_types::{Nonce, StorageKey, H256, U256};
use ola_vm::errors::{TxRevertReason, VmRevertReason};
use ola_vm::vm::VmExecutionResult;
use ola_vm::vm_with_bootloader::TxExecutionMode;

use super::{apply, BlockArgs};
use super::{error::SandboxExecutionError, TxSharedArgs, VmPermit};

#[derive(Debug)]
pub(crate) struct TxExecutionArgs {
    pub execution_mode: TxExecutionMode,
    pub enforced_nonce: Option<Nonce>,
    pub added_balance: U256,
}

impl TxExecutionArgs {
    pub fn for_validation(tx: &L2Tx) -> Self {
        Self {
            execution_mode: TxExecutionMode::VerifyExecute,
            enforced_nonce: Some(tx.nonce()),
            added_balance: U256::zero(),
        }
    }

    fn for_eth_call(vm_execution_cache_misses_limit: Option<usize>) -> Self {
        let missed_storage_invocation_limit = vm_execution_cache_misses_limit.unwrap_or(usize::MAX);
        Self {
            execution_mode: TxExecutionMode::EthCall {
                missed_storage_invocation_limit,
            },
            enforced_nonce: None,
            added_balance: U256::zero(),
        }
    }
}

#[tracing::instrument(skip_all)]
pub(crate) async fn execute_tx_eth_call(
    vm_permit: VmPermit,
    shared_args: TxSharedArgs,
    connection_pool: ConnectionPool,
    mut tx: L2Tx,
    block_args: BlockArgs,
    vm_execution_cache_misses_limit: Option<usize>,
    trace_call: bool,
) -> Result<VmExecutionResult, SandboxExecutionError> {
    let execution_args = TxExecutionArgs::for_eth_call(vm_execution_cache_misses_limit);

    let (vm_result, _) = execute_tx_in_sandbox2(
        vm_permit,
        shared_args,
        execution_args,
        connection_pool,
        tx.into(),
        block_args,
        // BootloaderJobType::TransactionExecution,
        trace_call,
        &mut HashMap::new(),
    )
    .await;

    vm_result
}

#[tracing::instrument(skip_all)]
pub(crate) async fn execute_tx_with_pending_state(
    vm_permit: VmPermit,
    shared_args: TxSharedArgs,
    connection_pool: ConnectionPool,
    tx: Transaction,
) -> Result<(), SandboxExecutionError> {
    let mut connection = connection_pool.access_storage_tagged("api").await;
    let block_args = BlockArgs::pending(&mut connection).await;
    drop(connection);

    execute_tx_in_sandbox(vm_permit, shared_args, connection_pool, tx, block_args).await
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip_all)]
async fn execute_tx_in_sandbox(
    vm_permit: VmPermit,
    shared_args: TxSharedArgs,
    connection_pool: ConnectionPool,
    tx: Transaction,
    block_args: BlockArgs,
) -> Result<(), SandboxExecutionError> {
    let execution_result = tokio::task::spawn_blocking(move || {
        let result =
            apply::apply_vm_in_sandbox(vm_permit, shared_args, &connection_pool, tx, block_args);
        result
    })
    .await
    .unwrap();

    execution_result.map_err(|e| {
        let revert_reason = VmRevertReason::General {
            msg: e.to_string(),
            data: vec![],
        };
        TxRevertReason::TxReverted(revert_reason).into()
    })
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip_all)]
async fn execute_tx_in_sandbox2(
    vm_permit: VmPermit,
    shared_args: TxSharedArgs,
    execution_args: TxExecutionArgs,
    connection_pool: ConnectionPool,
    tx: Transaction,
    block_args: BlockArgs,
    // job_type: BootloaderJobType,
    trace_call: bool,
    storage_read_cache: &mut HashMap<StorageKey, H256>,
) -> () {
    let total_factory_deps = tx
        .execute
        .factory_deps
        .as_ref()
        .map_or(0, |deps| deps.len() as u16);

    let moved_cache = std::mem::take(storage_read_cache);
    tokio::task::spawn_blocking(move || {
        // let span = span!(Level::DEBUG, "execute_in_sandbox").entered();
        let execution_mode = execution_args.execution_mode;
        let result = apply::apply_vm_in_sandbox(
            vm_permit,
            shared_args,
            &execution_args,
            &connection_pool,
            tx,
            block_args,
            moved_cache,
            |vm, tx| {
                push_transaction_to_bootloader_memory(vm, &tx, execution_mode, None);
                let result = if trace_call {
                    vm.execute_till_block_end_with_call_tracer(job_type)
                } else {
                    vm.execute_till_block_end(job_type)
                };
                result.full_result
            },
        );
        span.exit();
        result
    })
    .await
    .unwrap();

    *storage_read_cache = moved_cache;

    let tx_execution_metrics =
        vm_metrics::collect_tx_execution_metrics(total_factory_deps, &execution_result);
    let result = match execution_result.revert_reason {
        None => Ok(execution_result),
        Some(revert) => Err(revert.revert_reason.into()),
    };
    // (result, tx_execution_metrics)
    ()
}
