use ola_dal::connection::ConnectionPool;
use ola_types::{l2::L2Tx, Transaction};
use ola_types::{Nonce, U256};
use ola_vm::errors::{TxRevertReason, VmRevertReason};
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
