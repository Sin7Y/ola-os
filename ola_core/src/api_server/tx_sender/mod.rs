use std::{fmt::Debug, num::NonZeroU32, sync::Arc, time::Instant};

use governor::{
    clock::MonotonicClock,
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use ola_basic_types::Address;
use ola_config::{api::Web3JsonRpcConfig, sequencer::SequencerConfig};
use ola_contracts::BaseSystemContracts;
use ola_dal::connection::ConnectionPool;
use ola_state::postgres::PostgresStorageCaches;
use ola_types::H256;

use self::proxy::TxProxy;

use super::execution_sandbox::{VmConcurrencyBarrier, VmConcurrencyLimiter};

pub mod proxy;

pub struct ApiContracts {
    eth_call: BaseSystemContracts,
}

impl ApiContracts {
    pub fn load_from_disk() -> Self {
        // FIXME: replace playground
        Self {
            eth_call: BaseSystemContracts::playground(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TxSenderConfig {
    pub fee_account_addr: Address,
    pub max_nonce_ahead: u32,
    pub vm_execution_cache_misses_limit: Option<usize>,
    pub default_aa: H256,
    pub entrypoint: H256,
}

impl TxSenderConfig {
    pub fn new(sequencer_config: &SequencerConfig, web3_json_config: &Web3JsonRpcConfig) -> Self {
        Self {
            fee_account_addr: sequencer_config.fee_account_addr,
            max_nonce_ahead: web3_json_config.max_nonce_ahead,
            vm_execution_cache_misses_limit: web3_json_config.vm_execution_cache_misses_limit,
            default_aa: sequencer_config.default_aa_hash,
            entrypoint: sequencer_config.entrypoint_hash,
        }
    }
}

pub struct TxSender(pub(super) Arc<TxSenderInner>);

impl Clone for TxSender {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Debug for TxSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxSender").finish()
    }
}

type TxSenderRateLimiter =
    RateLimiter<NotKeyed, InMemoryState, MonotonicClock, NoOpMiddleware<Instant>>;

pub struct TxSenderInner {
    pub(super) sender_config: TxSenderConfig,
    pub master_connection_pool: Option<ConnectionPool>,
    pub replica_connection_pool: ConnectionPool,
    pub(super) api_contracts: ApiContracts,
    rate_limiter: Option<TxSenderRateLimiter>,
    pub(super) proxy: Option<TxProxy>,
    sequencer_config: Option<SequencerConfig>,
    pub(super) vm_concurrency_limiter: Arc<VmConcurrencyLimiter>,
    storage_caches: PostgresStorageCaches,
}

#[derive(Debug)]
pub struct TxSenderBuilder {
    config: TxSenderConfig,
    master_connection_pool: Option<ConnectionPool>,
    replica_connection_pool: ConnectionPool,
    rate_limiter: Option<TxSenderRateLimiter>,
    proxy: Option<TxProxy>,
    sequencer_config: Option<SequencerConfig>,
}

impl TxSenderBuilder {
    pub fn new(config: TxSenderConfig, replica_connection_pool: ConnectionPool) -> Self {
        Self {
            config,
            master_connection_pool: None,
            replica_connection_pool,
            rate_limiter: None,
            proxy: None,
            sequencer_config: None,
        }
    }

    pub fn with_rate_limiter(self, limit: u32) -> Self {
        let rate_limiter = RateLimiter::direct_with_clock(
            Quota::per_second(NonZeroU32::new(limit).unwrap()),
            &MonotonicClock::default(),
        );
        Self {
            rate_limiter: Some(rate_limiter),
            ..self
        }
    }

    pub fn with_tx_proxy(mut self, main_node_url: &str) -> Self {
        self.proxy = Some(TxProxy::new(main_node_url));
        self
    }

    pub fn with_main_connection_pool(mut self, master_connection_pool: ConnectionPool) -> Self {
        self.master_connection_pool = Some(master_connection_pool);
        self
    }

    pub fn with_sequencer_config(mut self, sequencer_config: SequencerConfig) -> Self {
        self.sequencer_config = Some(sequencer_config);
        self
    }

    pub async fn build(
        self,
        vm_concurrency_limiter: Arc<VmConcurrencyLimiter>,
        api_contracts: ApiContracts,
        storage_caches: PostgresStorageCaches,
    ) -> TxSender {
        TxSender(Arc::new(TxSenderInner {
            sender_config: self.config,
            master_connection_pool: self.master_connection_pool,
            replica_connection_pool: self.replica_connection_pool,
            api_contracts,
            rate_limiter: self.rate_limiter,
            proxy: self.proxy,
            sequencer_config: self.sequencer_config,
            vm_concurrency_limiter,
            storage_caches,
        }))
    }
}
