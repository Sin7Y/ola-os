use std::collections::HashMap;

use ola_dal::connection::ConnectionPool;
use ola_types::{StorageKey, Transaction, H256};

use super::{execute::TxExecutionArgs, BlockArgs, TxSharedArgs, VmPermit};

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_vm_in_sandbox<T>(
    vm_permit: VmPermit,
    shared_args: TxSharedArgs,
    execution_args: &TxExecutionArgs,
    connection_pool: &ConnectionPool,
    tx: Transaction,
    block_args: BlockArgs,
    storage_read_cache: HashMap<StorageKey, H256>,
    apply: impl FnOnce(Transaction) -> T,
) -> (T, HashMap<StorageKey, H256>) {
    // TODO:
    let result = apply(tx);
    (result, HashMap::default())
}
