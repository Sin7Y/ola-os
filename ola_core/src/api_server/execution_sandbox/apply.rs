use std::collections::HashMap;

use ola_dal::connection::ConnectionPool;
use ola_types::{StorageKey, Transaction, H256};

use super::{execute::TxExecutionArgs, BlockArgs, TxSharedArgs, VmPermit};

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_vm_in_sandbox<T>(
    _vm_permit: VmPermit,
    _shared_args: TxSharedArgs,
    _execution_args: &TxExecutionArgs,
    _connection_pool: &ConnectionPool,
    tx: Transaction,
    _block_args: BlockArgs,
    _storage_read_cache: HashMap<StorageKey, H256>,
    apply: impl FnOnce(Transaction) -> T,
) -> (T, HashMap<StorageKey, H256>) {
    // TODO:
    let result = apply(tx);
    (result, HashMap::default())
}
