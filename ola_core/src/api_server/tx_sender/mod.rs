use std::{fmt::Debug, num::NonZeroU32, sync::Arc, time::Instant};

use governor::{
    clock::MonotonicClock,
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use ola_config::{
    api::Web3JsonRpcConfig,
    constants::MAX_NEW_FACTORY_DEPS,
    database::load_db_config,
    sequencer::{load_network_config, SequencerConfig},
};
use ola_contracts::BaseSystemContracts;
use ola_dal::{connection::ConnectionPool, transactions_dal::L2TxSubmissionResult};
use ola_executor::{
    batch_exe_manager::BlockExeInfo,
    config::ExecuteMode,
    ola_storage::OlaCachedStorage,
    tx_exe_manager::{OlaTapeInitInfo, TxExeManager},
};
use ola_state::postgres::PostgresStorageCaches;
use ola_types::{
    fee::TransactionExecutionMetrics, l2::L2Tx, AccountTreeId, Address, Bytes, Nonce, H256,
};
use ola_utils::{bytes_to_u64s, time::millis_since_epoch, u64s_to_bytes};
use olavm_core::util::converts::u8_arr_to_address;

use self::{error::SubmitTxError, proxy::TxProxy};

use super::execution_sandbox::{TxSharedArgs, VmConcurrencyLimiter};

pub mod error;
pub mod proxy;

pub struct ApiContracts {
    eth_call: BaseSystemContracts,
}

impl ApiContracts {
    pub fn load_from_disk() -> Self {
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

impl TxSender {
    #[olaos_logs::instrument(skip(self, tx))]
    pub async fn submit_tx(&self, tx: L2Tx) -> Result<L2TxSubmissionResult, SubmitTxError> {
        olaos_logs::info!("Start submit tx {:?}", tx.hash());

        if let Some(rate_limiter) = &self.0.rate_limiter {
            if rate_limiter.check().is_err() {
                olaos_logs::info!("return RateLimitExceeded");
                return Err(SubmitTxError::RateLimitExceeded);
            }
        }

        olaos_logs::info!("start validate tx");

        self.validate_tx(&tx).await?;

        olaos_logs::info!("validate tx succeeded");

        let vm_permit = self.0.vm_concurrency_limiter.acquire().await;
        let vm_permit = vm_permit.ok_or(SubmitTxError::ServerShuttingDown)?;

        olaos_logs::info!("Acquired vm_permit");

        // let res = execute_tx_with_pending_state(
        //     vm_permit,
        //     shared_args.clone(),
        //     self.0.replica_connection_pool.clone(),
        //     tx.clone().into(),
        // )
        // .await;

        // if let Some(e) = res.err() {
        //     return Err(SubmitTxError::PreExecutionReverted(e.to_string(), vec![]));
        // }

        if let Some(proxy) = &self.0.proxy {
            olaos_logs::error!("proxy not supported");
            // We're running an external node: we have to proxy the transaction to the main node.
            // But before we do that, save the tx to cache in case someone will request it
            // Before it reaches the main node.
            proxy.save_tx(tx.hash(), tx.clone()).await;
            proxy.submit_tx(&tx).await?;
            // Now, after we are sure that the tx is on the main node, remove it from cache
            // since we don't want to store txs that might have been replaced or otherwise removed
            // from the mempool.
            proxy.forget_tx(tx.hash()).await;
            return Ok(L2TxSubmissionResult::Proxied);
        } else {
            assert!(
                self.0.master_connection_pool.is_some(),
                "TxSender is instantiated without both master connection pool and tx proxy"
            );
        }

        let nonce = tx.common_data.nonce.0;
        let hash = tx.hash();
        let expected_nonce = self.get_expected_nonce(&tx).await;

        olaos_logs::info!(
            "Got nonce {:?} and expected nonce {:?}",
            nonce,
            expected_nonce
        );

        let submission_res_handle = self
            .0
            .master_connection_pool
            .as_ref()
            .unwrap() // Checked above
            .access_storage_tagged("api")
            .await
            .transactions_dal()
            .insert_transaction_l2(tx, TransactionExecutionMetrics::default())
            .await;

        olaos_logs::info!("Try to insert tx into db");

        drop(vm_permit);

        olaos_logs::info!("Drop vm_permit");

        let res = match submission_res_handle {
            L2TxSubmissionResult::AlreadyExecuted => Err(SubmitTxError::NonceIsTooLow(
                expected_nonce.0,
                expected_nonce.0 + self.0.sender_config.max_nonce_ahead,
                nonce,
            )),
            L2TxSubmissionResult::Duplicate => Err(SubmitTxError::IncorrectTx(
                ola_types::l2::error::TxCheckError::TxDuplication(hash),
            )),
            _ => Ok(submission_res_handle),
        };

        olaos_logs::info!("Insert tx into db result {:?}", res);

        res
    }

    #[olaos_logs::instrument(skip(self, tx))]
    pub async fn call_transaction_impl(&self, tx: L2Tx) -> Result<Bytes, SubmitTxError> {
        olaos_logs::info!(
            "Start call tx from {:?}, to {:?}",
            tx.initiator_account(),
            tx.recipient_account()
        );

        let vm_permit = self.0.vm_concurrency_limiter.acquire().await;
        let vm_permit = vm_permit.ok_or(SubmitTxError::ServerShuttingDown)?;

        olaos_logs::info!("Acquired vm_permit, start prepare params");

        let mut storage = self
            .0
            .replica_connection_pool
            .access_storage_tagged("api")
            .await;

        let l1_batch_header = storage.blocks_dal().get_newest_l1_batch_header().await;

        let db_config = load_db_config().expect("failed to load database config");
        let network = load_network_config().expect("failed to load network config");

        olaos_logs::info!("Start call in vm_manager");

        let mut storage = OlaCachedStorage::new(
            db_config.sequencer_db_path,
            Some((millis_since_epoch() / 1_000) as u64),
        )
        .map_err(|e| SubmitTxError::TxCallTxError(e.to_string()))?;

        let block_info = BlockExeInfo {
            block_number: *l1_batch_header.number as u64 + 1,
            block_timestamp: (millis_since_epoch() / 1_000) as u64,
            sequencer_address: u8_arr_to_address(
                &self.0.sender_config.fee_account_addr.to_fixed_bytes(),
            ),
            chain_id: network.ola_network_id as u64,
        };
        let tape_init_info = OlaTapeInitInfo {
            version: tx.common_data.transaction_type as u64,
            origin_address: u8_arr_to_address(&tx.common_data.initiator_address.to_fixed_bytes()),
            calldata: bytes_to_u64s(tx.execute.calldata),
            nonce: None,
            signature_r: None,
            signature_s: None,
            tx_hash: None,
        };
        let mut tx_exe_manager: TxExeManager = TxExeManager::new(
            ExecuteMode::Call,
            block_info,
            tape_init_info,
            &mut storage,
            u8_arr_to_address(&tx.execute.contract_address.to_fixed_bytes()),
            0,
        );
        let call_res = tx_exe_manager
            .call()
            .map_err(|e| SubmitTxError::TxCallTxError(e.to_string()))?;
        let ret = u64s_to_bytes(&call_res);

        drop(vm_permit);

        olaos_logs::info!("Drop vm_permit");

        Ok(Bytes(ret))
    }

    async fn validate_tx(&self, tx: &L2Tx) -> Result<(), SubmitTxError> {
        if tx.execute.factory_deps_length() > MAX_NEW_FACTORY_DEPS {
            return Err(SubmitTxError::TooManyFactoryDependencies(
                tx.execute.factory_deps_length(),
                MAX_NEW_FACTORY_DEPS,
            ));
        }
        self.validate_account_nonce(tx).await?;
        Ok(())
    }

    async fn validate_account_nonce(&self, tx: &L2Tx) -> Result<(), SubmitTxError> {
        let expected_nonce = self.get_expected_nonce(tx).await;

        if tx.common_data.nonce.0 < expected_nonce.0 {
            Err(SubmitTxError::NonceIsTooLow(
                expected_nonce.0,
                expected_nonce.0 + self.0.sender_config.max_nonce_ahead,
                tx.nonce().0,
            ))
        } else {
            let max_nonce = expected_nonce.0 + self.0.sender_config.max_nonce_ahead;
            if !(expected_nonce.0..=max_nonce).contains(&tx.common_data.nonce.0) {
                Err(SubmitTxError::NonceIsTooHigh(
                    expected_nonce.0,
                    max_nonce,
                    tx.nonce().0,
                ))
            } else {
                Ok(())
            }
        }
    }

    async fn get_expected_nonce(&self, tx: &L2Tx) -> Nonce {
        let mut connection = self
            .0
            .replica_connection_pool
            .access_storage_tagged("api")
            .await;

        let latest_block_number = connection
            .blocks_web3_dal()
            .get_sealed_miniblock_number()
            .await
            .unwrap();
        let nonce = connection
            .storage_web3_dal()
            .get_address_historical_nonce(tx.initiator_account(), latest_block_number)
            .await
            .unwrap();
        Nonce(nonce.as_u32())
    }

    fn shared_args(&self) -> TxSharedArgs {
        TxSharedArgs {
            operator_account: AccountTreeId::new(self.0.sender_config.fee_account_addr),
            base_system_contracts: self.0.api_contracts.eth_call.clone(),
            caches: self.storage_caches(),
        }
    }

    pub(crate) fn storage_caches(&self) -> PostgresStorageCaches {
        self.0.storage_caches.clone()
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
            &MonotonicClock,
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
