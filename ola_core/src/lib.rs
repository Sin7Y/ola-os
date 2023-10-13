use std::sync::Arc;

use api_server::{
    execution_sandbox::{VmConcurrencyBarrier, VmConcurrencyLimiter},
    healthcheck::HealthCheckHandle,
    tx_sender::{ApiContracts, TxSender, TxSenderBuilder, TxSenderConfig},
    web3::{self, state::InternalApiconfig, Namespace},
};
use futures::channel::oneshot;
use ola_config::{
    api::{
        load_api_config, load_healthcheck_config, load_web3_json_rpc_config, ApiConfig,
        Web3JsonRpcConfig,
    },
    database::{load_db_config, DBConfig},
    sequencer::{NetworkConfig, SequencerConfig},
};
use ola_dal::{
    connection::{ConnectionPool, DbVariant},
    healthcheck::ConnectionPoolHealthCheck,
};
use ola_state::postgres::PostgresStorageCaches;
use olaos_health_check::{CheckHealth, ReactiveHealthCheck};
use tokio::{sync::watch, task::JoinHandle};

pub mod api_server;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Component {
    HttpApi,
    WsApi,
    Sequencer,
}
pub async fn initialize_components(
    components: Vec<Component>,
) -> anyhow::Result<(Vec<JoinHandle<()>>, watch::Sender<bool>, HealthCheckHandle)> {
    // FIXME:
    // let db_config = DBConfig::from_env();
    let db_config = load_db_config().expect("failed to load database config");

    let connection_pool = ConnectionPool::builder(DbVariant::Master).build().await;
    let replica_connection_pool = ConnectionPool::builder(DbVariant::Replica)
        .set_statement_timeout(db_config.statement_timeout())
        .build()
        .await;
    let mut healthchecks: Vec<Box<dyn CheckHealth>> = Vec::new();
    let (stop_sender, stop_receiver) = watch::channel(false);

    let mut task_futures: Vec<JoinHandle<()>> = vec![];

    if components.contains(&Component::WsApi) || components.contains(&Component::HttpApi) {
        // TODO:
        // let api_config = ApiConfig::from_env();
        let api_config = load_api_config().expect("failed to load api config");
        let sequencer_config = SequencerConfig::from_env();
        let network_config = NetworkConfig::from_env();
        let internal_api_config =
            InternalApiconfig::new(&network_config, &api_config.web3_json_rpc);
        let tx_sender_config = TxSenderConfig::new(&sequencer_config, &api_config.web3_json_rpc);
        let mut storage_caches = None;

        if components.contains(&Component::HttpApi) {
            storage_caches = Some(build_storage_caches(
                &replica_connection_pool,
                &mut task_futures,
            ));

            let (futures, health_check) = run_http_api(
                &api_config,
                &sequencer_config,
                &internal_api_config,
                &tx_sender_config,
                connection_pool.clone(),
                replica_connection_pool.clone(),
                stop_receiver.clone(),
                storage_caches.clone().unwrap(),
            )
            .await;
            task_futures.extend(futures);
            healthchecks.push(Box::new(health_check));
        }
    }
    healthchecks.push(Box::new(ConnectionPoolHealthCheck::new(
        replica_connection_pool,
    )));
    let healtcheck_api_config =
        load_healthcheck_config().expect("failed to load health_check config");
    let health_check_handle =
        HealthCheckHandle::spawn_server(healtcheck_api_config.bind_addr(), healthchecks);
    Ok((task_futures, stop_sender, health_check_handle))
}

async fn run_http_api(
    api_config: &ApiConfig,
    sequencer_config: &SequencerConfig,
    internal_api: &InternalApiconfig,
    tx_sender_config: &TxSenderConfig,
    master_connection_pool: ConnectionPool,
    replica_connection_pool: ConnectionPool,
    stop_receiver: watch::Receiver<bool>,
    storage_caches: PostgresStorageCaches,
) -> (Vec<JoinHandle<()>>, ReactiveHealthCheck) {
    let (tx_sender, vm_barrier) = build_tx_sender(
        tx_sender_config,
        &api_config.web3_json_rpc,
        sequencer_config,
        master_connection_pool,
        replica_connection_pool.clone(),
        storage_caches,
    )
    .await;

    let namespaces = Namespace::ALL.to_vec();

    web3::ApiBuilder::new(internal_api.clone(), replica_connection_pool)
        .http(api_config.web3_json_rpc.http_port)
        .with_filters_limit(api_config.web3_json_rpc.filters_limit())
        .with_threads(api_config.web3_json_rpc.http_server_threads())
        .with_batch_request_size_limit(api_config.web3_json_rpc.max_batch_request_size())
        .with_response_body_size_limit(api_config.web3_json_rpc.max_response_body_size())
        .with_tx_sender(tx_sender, vm_barrier)
        .enable_api_namespaces(namespaces)
        .build(stop_receiver.clone())
        .await
}

async fn build_tx_sender(
    tx_sender_config: &TxSenderConfig,
    web3_json_config: &Web3JsonRpcConfig,
    sequencer_config: &SequencerConfig,
    master_pool: ConnectionPool,
    replica_pool: ConnectionPool,
    storage_caches: PostgresStorageCaches,
) -> (TxSender, VmConcurrencyBarrier) {
    let mut tx_sender_builder = TxSenderBuilder::new(tx_sender_config.clone(), replica_pool)
        .with_main_connection_pool(master_pool)
        .with_sequencer_config(sequencer_config.clone());

    if let Some(transactions_per_sec_limit) = web3_json_config.transactions_per_sec_limit {
        tx_sender_builder = tx_sender_builder.with_rate_limiter(transactions_per_sec_limit);
    }

    let max_concurrency = web3_json_config.vm_concurrency_limit();
    let (vm_concurrency_limiter, vm_barrier) = VmConcurrencyLimiter::new(max_concurrency);

    let tx_sender = tx_sender_builder
        .build(
            Arc::new(vm_concurrency_limiter),
            ApiContracts::load_from_disk(),
            storage_caches,
        )
        .await;
    (tx_sender, vm_barrier)
}

fn build_storage_caches(
    replica_connection_pool: &ConnectionPool,
    task_futures: &mut Vec<JoinHandle<()>>,
) -> PostgresStorageCaches {
    // TODO:
    // let rpc_config = Web3JsonRpcConfig::from_env();
    let rpc_config = load_web3_json_rpc_config().expect("failed to load web3_json_rpc_config");
    let factory_deps_capacity = rpc_config.factory_deps_cache_size() as u64;
    let initial_writes_capacity = rpc_config.initial_writes_cache_size() as u64;
    let values_capacity = rpc_config.latest_values_cache_size() as u64;
    let mut storage_caches =
        PostgresStorageCaches::new(factory_deps_capacity, initial_writes_capacity);
    if values_capacity > 0 {
        let values_cache_task = storage_caches.configure_storage_values_cache(
            values_capacity,
            replica_connection_pool.clone(),
            tokio::runtime::Handle::current(),
        );
        task_futures.push(tokio::task::spawn_blocking(values_cache_task));
    }
    storage_caches
}

pub fn setup_sigint_handler() -> oneshot::Receiver<()> {
    let (sigint_sender, sigint_receiver) = oneshot::channel();
    let mut sigint_sender = Some(sigint_sender);
    ctrlc::set_handler(move || {
        if let Some(sigint_sender) = sigint_sender.take() {
            sigint_sender.send(()).ok();
            // ^ The send fails if `sigint_receiver` is dropped. We're OK with this,
            // since at this point the node should be stopping anyway, or is not interested
            // in listening to interrupt signals.
        }
    })
    .expect("Error setting Ctrl+C handler");

    sigint_receiver
}
