use std::collections::HashMap;

use ola_dal::connection::ConnectionPool;
use ola_types::{fee::TransactionExecutionMetrics, l2::L2Tx, StorageKey, Transaction};
use ola_types::{Nonce, H256, U256};
use ola_vm::vm_with_bootloader::BootloaderJobType;
use ola_vm::{vm::VmExecutionResult, vm_with_bootloader::TxExecutionMode};
use tracing::Level;

use super::{apply, vm_metrics, BlockArgs};
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
}

#[tracing::instrument(skip_all)]
pub(crate) async fn execute_tx_with_pending_state(
    vm_permit: VmPermit,
    mut shared_args: TxSharedArgs,
    execution_args: TxExecutionArgs,
    connection_pool: ConnectionPool,
    tx: Transaction,
    storage_read_cache: &mut HashMap<StorageKey, H256>,
) -> (
    Result<VmExecutionResult, SandboxExecutionError>,
    TransactionExecutionMetrics,
) {
    let mut connection = connection_pool.access_storage_tagged("api").await;
    let block_args = BlockArgs::pending(&mut connection).await;
    drop(connection);

    execute_tx_in_sandbox(
        vm_permit,
        shared_args,
        execution_args,
        connection_pool,
        tx,
        block_args,
        BootloaderJobType::TransactionExecution,
        false,
        storage_read_cache,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip_all)]
async fn execute_tx_in_sandbox(
    vm_permit: VmPermit,
    shared_args: TxSharedArgs,
    execution_args: TxExecutionArgs,
    connection_pool: ConnectionPool,
    tx: Transaction,
    block_args: BlockArgs,
    job_type: BootloaderJobType,
    trace_call: bool,
    storage_read_cache: &mut HashMap<StorageKey, H256>,
) -> (
    Result<VmExecutionResult, SandboxExecutionError>,
    TransactionExecutionMetrics,
) {
    let total_factory_deps = tx
        .execute
        .factory_deps
        .as_ref()
        .map_or(0, |deps| deps.len() as u16);

    let moved_cache = std::mem::take(storage_read_cache);
    let (execution_result, moved_cache) = tokio::task::spawn_blocking(move || {
        let span = tracing::span!(Level::DEBUG, "execute_in_sandbox").entered();
        let execution_mode = execution_args.execution_mode;
        let result = apply::apply_vm_in_sandbox(
            vm_permit,
            shared_args,
            &execution_args,
            &connection_pool,
            tx,
            block_args,
            moved_cache,
            // FIXME: replace apply return real VmExecutionResult
            |tx| VmExecutionResult::default(),
        );
        span.exit();
        result
    })
    .await
    .unwrap();

    *storage_read_cache = moved_cache;

    let tx_execution_metrics =
        vm_metrics::collect_tx_execution_metrics(total_factory_deps, &execution_result);
    // FIXME:
    let result = Ok(execution_result);
    // let result = match execution_result.revert_reason {
    //     None => Ok(execution_result),
    //     Some(revert) => Err(revert.revert_reason.into()),
    // };
    (result, tx_execution_metrics)
}
