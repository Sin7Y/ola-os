use std::time::Duration;

use ethereum_types::H256;

pub mod execute_contract;

#[derive(Debug)]
pub struct SyncTransactionHandle<'a, P> {
    hash: H256,
    provider: &'a P,
    polling_interval: Duration,
    commit_timeout: Option<Duration>,
    finalize_timeout: Option<Duration>,
}