use std::time::Duration;

use ethereum_types::H256;
use ola_web3_decl::namespaces::eth::EthNamespaceClient;

pub mod execute_contract;

#[derive(Debug)]
pub struct SyncTransactionHandle<'a, P> {
    hash: H256,
    provider: &'a P,
    polling_interval: Duration,
    commit_timeout: Option<Duration>,
    finalize_timeout: Option<Duration>,
}

impl<'a, P> SyncTransactionHandle<'a, P>
where
    P: EthNamespaceClient + Sync,
{
    pub fn new(hash: H256, provider: &'a P) -> Self {
        Self {
            hash,
            provider,
            polling_interval: Duration::from_secs(1),
            commit_timeout: None,
            finalize_timeout: None,
        }
    }

    pub fn hash(&self) -> H256 {
        self.hash
    }
}
