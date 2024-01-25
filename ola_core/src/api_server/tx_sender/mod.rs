use std::{collections::HashMap, fmt::Debug, num::NonZeroU32, sync::Arc, time::Instant};

use crate::sequencer::{seal_criteria::conditional_sealer::ConditionalSealer, SealData};
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
use ola_state::postgres::PostgresStorageCaches;
use ola_types::{
    fee::TransactionExecutionMetrics, l2::L2Tx, AccountTreeId, Address, Nonce, Transaction, H256,
};
use ola_utils::{time::millis_since_epoch, u64s_to_bytes};
use ola_web3_decl::error::Web3Error;

use self::{error::SubmitTxError, proxy::TxProxy};

use super::execution_sandbox::{
    execute::{execute_tx_with_pending_state, TxExecutionArgs},
    TxSharedArgs, VmConcurrencyLimiter,
};
use olavm_core::types::merkle_tree::h256_to_tree_key;
use olavm_core::types::{
    storage::{field_arr_to_u8_arr, u8_arr_to_field_arr},
    Field, GoldilocksField,
};
use olavm_core::vm::transaction::TxCtxInfo;
use web3::types::Bytes;
use zk_vm::{BlockInfo, CallInfo, OlaVM, VmManager as OlaVmManager};

pub mod error;
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

impl TxSender {
    #[tracing::instrument(skip(self, tx))]
    pub async fn submit_tx(&self, tx: L2Tx) -> Result<L2TxSubmissionResult, SubmitTxError> {
        if let Some(rate_limiter) = &self.0.rate_limiter {
            if rate_limiter.check().is_err() {
                return Err(SubmitTxError::RateLimitExceeded);
            }
        }

        self.validate_tx(&tx).await?;

        let shared_args = self.shared_args();
        let vm_permit = self.0.vm_concurrency_limiter.acquire().await;
        let vm_permit = vm_permit.ok_or(SubmitTxError::ServerShuttingDown)?;

        // TODO: @Pierre begin
        let (_, tx_metrics) = execute_tx_with_pending_state(
            vm_permit.clone(),
            shared_args.clone(),
            TxExecutionArgs::for_validation(&tx),
            self.0.replica_connection_pool.clone(),
            tx.clone().into(),
            &mut HashMap::new(),
        )
        .await;

        olaos_logs::info!(
            "Submit tx {:?} with execution metrics {:?}",
            tx.hash(),
            tx_metrics
        );

        let validation_result = shared_args
            .validate_tx_with_pending_state(
                vm_permit,
                self.0.replica_connection_pool.clone(),
                tx.clone(),
            )
            .await;

        if let Err(err) = validation_result {
            return Err(SubmitTxError::ValidationFailed(err));
        }

        self.ensure_tx_executable(tx.clone().into(), &tx_metrics, true)?;

        // TODO: @Pierre end

        if let Some(proxy) = &self.0.proxy {
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
        let submission_res_handle = self
            .0
            .master_connection_pool
            .as_ref()
            .unwrap() // Checked above
            .access_storage_tagged("api")
            .await
            .transactions_dal()
            .insert_transaction_l2(tx, tx_metrics)
            .await;

        match submission_res_handle {
            L2TxSubmissionResult::AlreadyExecuted => Err(SubmitTxError::NonceIsTooLow(
                expected_nonce.0,
                expected_nonce.0 + self.0.sender_config.max_nonce_ahead,
                nonce,
            )),
            L2TxSubmissionResult::Duplicate => Err(SubmitTxError::IncorrectTx(
                ola_types::l2::error::TxCheckError::TxDuplication(hash),
            )),
            _ => Ok(submission_res_handle),
        }
    }

    #[tracing::instrument(skip(self, tx))]
    pub async fn call_transaction_impl(&self, tx: L2Tx) -> Result<Bytes, SubmitTxError> {
        let mut storage = self
            .0
            .replica_connection_pool
            .access_storage_tagged("api")
            .await;

        let l1_batch_header = storage.blocks_dal().get_newest_l1_batch_header().await;

        let db_config = load_db_config().expect("failed to load database config");
        let network = load_network_config().expect("failed to load network config");

        let call_info = CallInfo {
            version: tx.common_data.transaction_type as u32,
            caller_address: tx.common_data.initiator_address.to_fixed_bytes(),
            calldata: tx.execute.calldata.clone(),
            to_address: tx.execute.contract_address.to_fixed_bytes(),
        };
        let block_info = BlockInfo {
            block_number: *l1_batch_header.number + 1,
            block_timestamp: (millis_since_epoch() / 1_000) as u64,
            sequencer_address: self.0.sender_config.fee_account_addr.to_fixed_bytes(),
            chain_id: network.ola_network_id,
        };
        let mut vm_manager = OlaVmManager::new(
            block_info,
            db_config.merkle_tree.path,
            db_config.sequencer_db_path,
        );
        let call_res = vm_manager
            .call(call_info)
            .map_err(|e| SubmitTxError::TxCallTxError(e.to_string()))?;
        let ret = u64s_to_bytes(&call_res);
        Ok(Bytes(ret))
    }

    // TODO: add more check
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

    fn ensure_tx_executable(
        &self,
        transaction: Transaction,
        tx_metrics: &TransactionExecutionMetrics,
        log_message: bool,
    ) -> Result<(), SubmitTxError> {
        let Some(sk_config) = &self.0.sequencer_config else {
            // No config provided, so we can't check if transaction satisfies the seal criteria.
            // We assume that it's executable, and if it's not, it will be caught by the main server
            // (where this check is always performed).
            return Ok(());
        };

        // Hash is not computable for the provided `transaction` during gas estimation (it doesn't have
        // its input data set). Since we don't log a hash in this case anyway, we just use a dummy value.
        let tx_hash = if log_message {
            transaction.hash()
        } else {
            H256::zero()
        };

        let seal_data = SealData::for_transaction(transaction, tx_metrics);
        if let Some(reason) = ConditionalSealer::find_unexecutable_reason(sk_config, &seal_data) {
            let message = format!(
                "Tx is Unexecutable because of {reason}; inputs for decision: {seal_data:?}"
            );
            if log_message {
                olaos_logs::info!("{tx_hash:#?} {message}");
            }
            return Err(SubmitTxError::Unexecutable(message));
        }
        Ok(())
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
