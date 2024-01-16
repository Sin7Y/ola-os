use std::{sync::Arc, time::Instant};

use anyhow::{Context, Ok};
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
    chain::{load_mempool_config, MempoolConfig, OperationsManagerConfig},
    contracts::{load_contracts_config, ContractsConfig},
    database::{load_db_config, DBConfig},
    object_store::load_object_store_config,
    proof_data_handler::load_proof_data_handler_config,
    sequencer::{load_network_config, load_sequencer_config, NetworkConfig, SequencerConfig},
};
use ola_contracts::BaseSystemContracts;
use ola_dal::{
    connection::{ConnectionPool, DbVariant},
    healthcheck::ConnectionPoolHealthCheck,
    StorageProcessor,
};
use ola_state::postgres::PostgresStorageCaches;
use ola_types::{system_contracts::get_system_smart_contracts, L2ChainId};
use olaos_health_check::{CheckHealth, ReactiveHealthCheck};
use olaos_object_store::ObjectStoreFactory;
use olaos_queued_job_processor::JobProcessor;
use sequencer::{
    create_sequencer, io::MiniblockSealer, mempool_actor::MempoolFetcher, types::MempoolGuard,
};
use tokio::{sync::watch, task::JoinHandle};
use witness_input_producer::WitnessInputProducer;

pub mod api_server;
pub mod genesis;
pub mod metadata_calculator;
pub mod proof_data_handler;
pub mod sequencer;
pub mod tests;
pub mod witness_input_producer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Component {
    HttpApi,
    WsApi,
    Sequencer,
    Tree,
    WitnessInputProducer,
    ProofDataHandler,
}
pub async fn initialize_components(
    components: Vec<Component>,
) -> anyhow::Result<(
    Vec<JoinHandle<anyhow::Result<()>>>,
    watch::Sender<bool>,
    HealthCheckHandle,
)> {
    olaos_logs::info!("Starting the components: {components:?}");
    let db_config = load_db_config().expect("failed to load database config");
    let connection_pool = ConnectionPool::builder(DbVariant::Master).build().await;
    let replica_connection_pool = ConnectionPool::builder(DbVariant::Replica)
        .set_statement_timeout(db_config.statement_timeout())
        .build()
        .await;

    let mut healthchecks: Vec<Box<dyn CheckHealth>> = Vec::new();

    let contracts_config = load_contracts_config().expect("failed to load contract config");

    let (stop_sender, stop_receiver) = watch::channel(false);

    let mut task_futures: Vec<JoinHandle<anyhow::Result<()>>> = vec![];

    if components.contains(&Component::HttpApi) {
        let api_config = load_api_config().expect("failed to load api config");
        let sequencer_config = load_sequencer_config().expect("failed to load sequencer config");
        let network_config = load_network_config().expect("failed to load network config");
        let tx_sender_config = TxSenderConfig::new(&sequencer_config, &api_config.web3_json_rpc);
        let internal_api_config = InternalApiconfig::new(
            &network_config,
            &api_config.web3_json_rpc,
            &contracts_config,
        );

        let mut storage_caches = None;

        if components.contains(&Component::HttpApi) {
            storage_caches = Some(build_storage_caches(
                &replica_connection_pool,
                &mut task_futures,
            ));

            let started_at = Instant::now();
            olaos_logs::info!("initializing HTTP API");
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
            olaos_logs::info!("initialized HTTP API in {:?}", started_at.elapsed());
        }
    }

    if components.contains(&Component::Sequencer) {
        let started_at = Instant::now();
        olaos_logs::info!("initializing Sequencer");
        let sequencer_config = load_sequencer_config().expect("failed to load sequencer config");
        let mempool_config = load_mempool_config().expect("failed to load mempool config");
        add_sequencer_to_task_futures(
            &mut task_futures,
            &contracts_config,
            sequencer_config,
            &db_config,
            &mempool_config,
            stop_receiver.clone(),
        )
        .await;
        olaos_logs::info!("initialized Sequencer in {:?}", started_at.elapsed());
    }

    if components.contains(&Component::Tree) {
        let started_at = Instant::now();
        olaos_logs::info!("initializing Merkle Tree");
        add_trees_to_task_futures(
            &mut task_futures,
            &mut healthchecks,
            &components,
            stop_receiver.clone(),
        )
        .await;
        olaos_logs::info!("initialized Merkle Tree in {:?}", started_at.elapsed());
    }

    let object_store_config =
        load_object_store_config().expect("failed to load object store config");
    let store_factory = ObjectStoreFactory::new(object_store_config);

    if components.contains(&Component::WitnessInputProducer) {
        let started_at = Instant::now();
        olaos_logs::info!("initializing Merkle Tree");
        let pool_builder = ConnectionPool::singleton(DbVariant::Master);
        let connection_pool = pool_builder.build().await;
        let network_config = load_network_config().expect("failed to load network config");
        let _ = add_witness_input_producer_to_task_futures(
            &mut task_futures,
            &connection_pool,
            &store_factory,
            L2ChainId(network_config.ola_network_id),
            stop_receiver.clone(),
        )
        .await
        .context("add_witness_input_producer_to_task_futures");
        olaos_logs::info!(
            "initialized WitnessInputProducer in {:?}",
            started_at.elapsed()
        );
    }

    if components.contains(&Component::ProofDataHandler) {
        let proof_data_handler_config =
            load_proof_data_handler_config().expect("failed to load proof data handler config");
        task_futures.push(tokio::spawn(proof_data_handler::run_server(
            proof_data_handler_config,
            store_factory.create_store().await,
            connection_pool.clone(),
            stop_receiver.clone(),
        )));
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
) -> (Vec<JoinHandle<anyhow::Result<()>>>, ReactiveHealthCheck) {
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
    task_futures: &mut Vec<JoinHandle<anyhow::Result<()>>>,
) -> PostgresStorageCaches {
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

async fn add_sequencer_to_task_futures(
    task_futures: &mut Vec<JoinHandle<anyhow::Result<()>>>,
    contracts_config: &ContractsConfig,
    sequencer_config: SequencerConfig,
    db_config: &DBConfig,
    mempool_config: &MempoolConfig,
    stop_receiver: watch::Receiver<bool>,
) {
    let pool_builder = ConnectionPool::singleton(DbVariant::Master);
    let sequencer_pool = pool_builder.build().await;
    let next_priority_id = sequencer_pool
        .access_storage()
        .await
        .transactions_dal()
        .next_priority_id()
        .await;
    let mempool = MempoolGuard::new(next_priority_id, mempool_config.capacity);

    let miniblock_sealer_pool = pool_builder.build().await;
    let (miniblock_sealer, miniblock_sealer_handle) = MiniblockSealer::new(
        miniblock_sealer_pool,
        sequencer_config.miniblock_seal_queue_capacity,
    );
    task_futures.push(tokio::spawn(miniblock_sealer.run()));

    let sequencer = create_sequencer(
        contracts_config,
        sequencer_config,
        db_config,
        mempool_config,
        sequencer_pool,
        mempool.clone(),
        miniblock_sealer_handle,
        stop_receiver.clone(),
    )
    .await;
    task_futures.push(tokio::spawn(sequencer.run()));

    let mempool_fetcher_pool = pool_builder.build().await;
    let mempool_fetcher = MempoolFetcher::new(mempool, mempool_config);
    let mempool_fetcher_handle = tokio::spawn(mempool_fetcher.run(
        mempool_fetcher_pool,
        mempool_config.remove_stuck_txs,
        mempool_config.stuck_tx_timeout(),
        stop_receiver,
    ));
    task_futures.push(mempool_fetcher_handle);
}

pub async fn genesis_init(network_config: &NetworkConfig, _contracts_config: &ContractsConfig) {
    let mut storage: StorageProcessor<'_> = StorageProcessor::establish_connection(true).await;

    genesis::ensure_genesis_state(
        &mut storage,
        L2ChainId(network_config.ola_network_id),
        &genesis::GenesisParams {
            base_system_contracts: BaseSystemContracts::load_from_disk(),
            system_contracts: get_system_smart_contracts(),
        },
    )
    .await;
}

pub async fn is_genesis_needed() -> bool {
    let mut storage = StorageProcessor::establish_connection(true).await;
    storage.blocks_dal().is_genesis_needed().await
}

async fn add_trees_to_task_futures(
    task_futures: &mut Vec<JoinHandle<anyhow::Result<()>>>,
    healthchecks: &mut Vec<Box<dyn CheckHealth>>,
    _components: &[Component],
    stop_receiver: watch::Receiver<bool>,
) {
    let db_config = DBConfig::from_env();
    let operation_config = OperationsManagerConfig::from_env();
    let (future, tree_health_check) = run_tree(&db_config, &operation_config, stop_receiver).await;
    task_futures.push(future);
    healthchecks.push(Box::new(tree_health_check));
}

async fn run_tree(
    config: &DBConfig,
    operation_manager: &OperationsManagerConfig,
    stop_receiver: watch::Receiver<bool>,
) -> (JoinHandle<anyhow::Result<()>>, ReactiveHealthCheck) {
    let started_at = Instant::now();
    let config =
        metadata_calculator::MetadataCalculatorConfig::for_main_node(config, operation_manager);
    let metadata_calculator = metadata_calculator::MetadataCalculator::new(&config).await;
    let tree_health_check = metadata_calculator.tree_health_check();
    let pool = ConnectionPool::singleton(DbVariant::Master).build().await;
    let prover_pool = ConnectionPool::singleton(DbVariant::Prover).build().await;
    let future = tokio::spawn(metadata_calculator.run(pool, prover_pool, stop_receiver));
    olaos_logs::info!("Initialized merkle tree in {:?}", started_at.elapsed());
    (future, tree_health_check)
}

async fn add_witness_input_producer_to_task_futures(
    task_futures: &mut Vec<JoinHandle<anyhow::Result<()>>>,
    connection_pool: &ConnectionPool,
    store_factory: &ObjectStoreFactory,
    l2_chain_id: L2ChainId,
    stop_receiver: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let started_at = Instant::now();
    olaos_logs::info!("initializing WitnessInputProducer");
    let producer =
        WitnessInputProducer::new(connection_pool.clone(), store_factory, l2_chain_id).await?;
    task_futures.push(tokio::spawn(producer.run(stop_receiver, None)));
    olaos_logs::info!(
        "Initialized WitnessInputProducer in {:?}",
        started_at.elapsed()
    );
    Ok(())
}
