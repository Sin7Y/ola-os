use std::{net::SocketAddr, num::NonZeroU32, time::Duration};

use anyhow::{Context, Ok};
use futures::future;
use jsonrpsee::{
    server::{BatchRequestConfig, RpcServiceBuilder, ServerBuilder},
    RpcModule,
};
use ola_dal::{connection::ConnectionPool, StorageProcessor};
use ola_types::{api::BlockId, MiniblockNumber};
use ola_web3_decl::{
    error::Web3Error,
    namespaces::{
        eth::{EthNamespaceServer, EthPubSubServer},
        ola::OlaNamespaceServer,
    },
};
use olaos_health_check::{HealthStatus, HealthUpdater, ReactiveHealthCheck};
use serde::Deserialize;
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};
use tower_http::{cors::CorsLayer, metrics::InFlightRequestsLayer};

use crate::{
    api_server::web3::{
        backend::batch_limiter_middleware::LimitMiddleware, pubsub::EthSubscriptionIdProvider,
    },
    utils::wait_for_l1_batch,
};

use self::{
    backend::error::internal_error,
    namespaces::{eth::EthNamespace, ola::OlaNamespace},
    pubsub::{EthSubscribe, PubSubEvent},
    state::{InternalApiConfig, RpcState},
};

use super::{execution_sandbox::VmConcurrencyBarrier, tx_sender::TxSender};
use crate::api_server::execution_sandbox::BlockStartInfo;

pub mod backend;
pub mod namespaces;
pub mod pubsub;
pub mod state;
#[cfg(test)]
pub(crate) mod tests;

const SERVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy)]
enum ApiTransport {
    WebSocket(SocketAddr),
    Http(SocketAddr),
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Namespace {
    Ola,
    Eth,
    Pubsub,
}

impl Namespace {
    pub const HTTP: &'static [Namespace] = &[Namespace::Ola, Namespace::Eth, Namespace::Pubsub];
}

/// Handles to the initialized API server.
#[derive(Debug)]
pub struct ApiServerHandles {
    pub tasks: Vec<JoinHandle<anyhow::Result<()>>>,
    pub health_check: ReactiveHealthCheck,
    // #[allow(unused)] // only used in tests
    // pub(crate) local_addr: future::TryMaybeDone<oneshot::Receiver<SocketAddr>>,
}

/// Optional part of the API server parameters.
#[derive(Debug, Default)]
struct OptionalApiParams {
    // sync_state: Option<SyncState>,
    filters_limit: Option<usize>,
    subscriptions_limit: Option<usize>,
    batch_request_size_limit: Option<usize>,
    response_body_size_limit: Option<usize>,
    websocket_requests_per_minute_limit: Option<NonZeroU32>,
    // tree_api_url: Option<String>,
    pub_sub_events_sender: Option<mpsc::UnboundedSender<PubSubEvent>>,
}

/// Full API server parameters.
#[derive(Debug)]
struct FullApiParams {
    pool: ConnectionPool,
    // last_miniblock_pool: ConnectionPool,
    config: InternalApiConfig,
    transport: ApiTransport,
    // tx_sender: TxSender,
    vm_barrier: VmConcurrencyBarrier,
    polling_interval: Duration,
    namespaces: Vec<Namespace>,
    optional: OptionalApiParams,
}

#[derive(Debug)]
pub struct ApiBuilder {
    pool: ConnectionPool,
    config: InternalApiConfig,
    transport: Option<ApiTransport>,
    tx_sender: Option<TxSender>,
    vm_barrier: Option<VmConcurrencyBarrier>,
    filters_limit: Option<usize>,
    subscriptions_limit: Option<usize>,
    batch_request_size_limit: Option<usize>,
    response_body_size_limit: Option<usize>,
    threads: Option<usize>,
    vm_concurrency_limit: Option<usize>,
    polling_interval: Option<Duration>,
    namespaces: Option<Vec<Namespace>>,
}

impl ApiBuilder {
    pub fn http_backend(config: InternalApiConfig, pool: ConnectionPool) -> Self {
        Self {
            transport: None,
            config,
            namespaces: None,
            threads: None,
            tx_sender: None,
            vm_barrier: None,
            batch_request_size_limit: None,
            response_body_size_limit: None,
            filters_limit: None,
            pool,
            subscriptions_limit: None,
            vm_concurrency_limit: None,
            polling_interval: None,
        }
    }

    pub fn pubsub_backend(config: InternalApiConfig, pool: ConnectionPool) -> Self {
        Self {
            transport: None,
            pool,
            tx_sender: None,
            vm_barrier: None,
            filters_limit: None,
            subscriptions_limit: None,
            batch_request_size_limit: None,
            response_body_size_limit: None,
            threads: None,
            vm_concurrency_limit: None,
            polling_interval: None,
            namespaces: None,
            config,
        }
    }

    pub fn ws(mut self, port: u16) -> Self {
        self.transport = Some(ApiTransport::WebSocket(([0, 0, 0, 0], port).into()));
        self
    }

    pub fn http(mut self, port: u16) -> Self {
        self.transport = Some(ApiTransport::Http(([0, 0, 0, 0], port).into()));
        self
    }

    pub fn with_filters_limit(mut self, filters_limit: usize) -> Self {
        self.filters_limit = Some(filters_limit);
        self
    }

    pub fn with_subscriptions_limit(mut self, subscriptions_limit: usize) -> Self {
        self.subscriptions_limit = Some(subscriptions_limit);
        self
    }

    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = Some(threads);
        self
    }

    pub fn with_polling_interval(mut self, polling_interval: Duration) -> Self {
        self.polling_interval = Some(polling_interval);
        self
    }

    pub fn with_batch_request_size_limit(mut self, limit: usize) -> Self {
        self.batch_request_size_limit = Some(limit);
        self
    }

    pub fn with_response_body_size_limit(mut self, limit: usize) -> Self {
        self.response_body_size_limit = Some(limit);
        self
    }

    pub fn with_tx_sender(mut self, tx_sender: TxSender, vm_barrier: VmConcurrencyBarrier) -> Self {
        self.tx_sender = Some(tx_sender);
        self.vm_barrier = Some(vm_barrier);
        self
    }

    pub fn with_vm_barrier(mut self, vm_barrier: VmConcurrencyBarrier) -> Self {
        self.vm_barrier = Some(vm_barrier);
        self
    }

    pub fn enable_api_namespaces(mut self, namespaces: Vec<Namespace>) -> Self {
        self.namespaces = Some(namespaces);
        self
    }
}

impl ApiBuilder {
    pub async fn build(
        mut self,
        stop_receiver: watch::Receiver<bool>,
    ) -> (
        Vec<tokio::task::JoinHandle<anyhow::Result<()>>>,
        ReactiveHealthCheck,
    ) {
        let transport = self.transport.clone();
        match transport {
            Some(ApiTransport::Http(addr)) => {
                let (api_health_check, health_updater) = ReactiveHealthCheck::new("http_api");
                (
                    vec![self.build_http(addr, stop_receiver, health_updater).await],
                    api_health_check,
                )
            }
            Some(ApiTransport::WebSocket(addr)) => {
                let (api_health_check, health_updater) = ReactiveHealthCheck::new("ws_api");
                (
                    vec![self.build_ws(addr, stop_receiver, health_updater).await],
                    api_health_check,
                )
            }
            None => panic!("ApiTransport is not specified"),
        }
    }

    pub async fn build_ws_new(
        self,
        stop_receiver: watch::Receiver<bool>,
    ) -> anyhow::Result<ApiServerHandles> {
        // self.into_full_params()?.spawn_server(stop_receiver).await
        self.build_jsonrpsee(stop_receiver).await
    }

    async fn build_http(
        self,
        addr: SocketAddr,
        stop_receiver: watch::Receiver<bool>,
        health_updater: HealthUpdater,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let rpc = self.build_rpc_module().await;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("ola-rpc-http-worker")
            .worker_threads(self.threads.unwrap())
            .build()
            .unwrap();
        let vm_barrier = self.vm_barrier.unwrap();
        let batch_request_config = if let Some(limit) = self.batch_request_size_limit {
            BatchRequestConfig::Limit(limit as u32)
        } else {
            BatchRequestConfig::Unlimited
        };
        let response_body_size_limit = self
            .response_body_size_limit
            .map(|limit| limit as u32)
            .unwrap_or(u32::MAX);
        tokio::task::spawn_blocking(move || {
            runtime.block_on(Self::run_rpc_server(
                true,
                rpc,
                addr,
                stop_receiver,
                health_updater,
                vm_barrier,
                batch_request_config,
                response_body_size_limit,
            ));
            runtime.shutdown_timeout(SERVER_SHUTDOWN_TIMEOUT);
            Ok(())
        })
    }

    async fn build_ws(
        self,
        addr: SocketAddr,
        stop_receiver: watch::Receiver<bool>,
        health_updater: HealthUpdater,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let rpc = self.build_rpc_module().await;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("ola-rpc-ws-worker")
            .worker_threads(self.threads.unwrap())
            .build()
            .unwrap();
        let vm_barrier = self.vm_barrier.unwrap();
        let batch_request_config = if let Some(limit) = self.batch_request_size_limit {
            BatchRequestConfig::Limit(limit as u32)
        } else {
            BatchRequestConfig::Unlimited
        };
        let response_body_size_limit = self
            .response_body_size_limit
            .map(|limit| limit as u32)
            .unwrap_or(u32::MAX);

        tokio::task::spawn_blocking(move || {
            runtime.block_on(Self::run_rpc_server(
                false,
                rpc,
                addr,
                stop_receiver,
                health_updater,
                vm_barrier,
                batch_request_config,
                response_body_size_limit,
            ));
            runtime.shutdown_timeout(SERVER_SHUTDOWN_TIMEOUT);
            Ok(())
        })
    }

    async fn build_jsonrpsee(
        self,
        stop_receiver: watch::Receiver<bool>,
    ) -> anyhow::Result<ApiServerHandles> {
        let transport = self.transport.expect("failed to specify transport");
        let health_check_name = match transport {
            ApiTransport::Http(_) => "http_api",
            ApiTransport::WebSocket(_) => "ws_api",
        };
        let (health_check, health_updater) = ReactiveHealthCheck::new(health_check_name);

        let mut tasks = vec![];
        let namespaces = self.namespaces.as_ref().unwrap();
        let pub_sub = if matches!(transport, ApiTransport::WebSocket(_))
            && namespaces.contains(&Namespace::Pubsub)
        {
            let pub_sub = EthSubscribe::new();
            // if let Some(sender) = &self.optional.pub_sub_events_sender {
            //     pub_sub.set_events_sender(sender.clone());
            // }

            tasks.extend(
                pub_sub.spawn_notifiers(
                    self.pool.clone(),
                    self.polling_interval
                        .expect("polling_interval not specified"),
                    stop_receiver.clone(),
                ),
            );
            Some(pub_sub)
        } else {
            None
        };
        // Start the server in a separate tokio runtime from a dedicated thread.
        let (local_addr_sender, local_addr) = oneshot::channel();
        let server_task = tokio::spawn(self.run_jsonrpsee_server(
            stop_receiver,
            pub_sub,
            local_addr_sender,
            health_updater,
        ));
        tasks.push(server_task);
        Ok(ApiServerHandles {
            health_check,
            tasks,
            // local_addr: future::try_maybe_done(local_addr),
        })
    }

    async fn run_jsonrpsee_server(
        self,
        mut stop_receiver: watch::Receiver<bool>,
        pub_sub: Option<EthSubscribe>,
        local_addr_sender: oneshot::Sender<SocketAddr>,
        health_updater: HealthUpdater,
    ) -> anyhow::Result<()> {
        let transport = self.transport.expect("transport is not specified");
        let (transport_str, is_http, addr) = match transport {
            ApiTransport::Http(addr) => ("HTTP", true, addr),
            ApiTransport::WebSocket(addr) => ("WS", false, addr),
        };

        olaos_logs::info!(
            "Waiting for at least one L1 batch in Postgres to start {transport_str} API server"
        );
        // Starting the server before L1 batches are present in Postgres can lead to some invariants the server logic
        // implicitly assumes not being upheld. The only case when we'll actually wait here is immediately after snapshot recovery.
        let polling_interval = self
            .polling_interval
            .expect("polling_interval is not specified");
        let earliest_l1_batch_number =
            wait_for_l1_batch(&self.pool, polling_interval, &mut stop_receiver)
                .await
                .context("error while waiting for L1 batch in Postgres")?;
        if let Some(number) = earliest_l1_batch_number {
            olaos_logs::info!("Successfully waited for at least one L1 batch in Postgres; the earliest one is #{number}");
        } else {
            olaos_logs::info!("Received shutdown signal before {transport_str} API server is started; shutting down");
            return Ok(());
        }

        let rpc = self.build_rpc_module_new(pub_sub).await?;
        // Setup CORS.
        let cors = is_http.then(|| {
            CorsLayer::new()
                // Allow `POST` when accessing the resource
                .allow_methods([reqwest::Method::POST])
                // Allow requests from any origin
                .allow_origin(tower_http::cors::Any)
                .allow_headers([reqwest::header::CONTENT_TYPE])
        });
        // Setup metrics for the number of in-flight requests.
        let transport = if is_http { "HTTP" } else { "WS" };
        let (in_flight_requests, counter) = InFlightRequestsLayer::pair();
        tokio::spawn(
            counter.run_emitter(Duration::from_millis(100), move |count| {
                metrics::histogram!("api.web3.in_flight_requests", count as f64, "scheme" => transport);
                future::ready(())
            }),
        );
        // Assemble server middleware.
        let middleware = tower::ServiceBuilder::new()
            .layer(in_flight_requests)
            .option_layer(cors);

        // Settings shared by HTTP and WS servers.
        let max_connections = !is_http
            .then_some(self.subscriptions_limit)
            .flatten()
            .unwrap_or(5_000);
        let batch_request_config = if let Some(limit) = self.batch_request_size_limit {
            BatchRequestConfig::Limit(limit as u32)
        } else {
            BatchRequestConfig::Unlimited
        };
        let server_builder = ServerBuilder::default()
            .max_connections(max_connections as u32)
            .set_http_middleware(middleware)
            .max_response_body_size(
                self.response_body_size_limit
                    .expect("response_body_size_limit not specified") as u32,
            )
            .set_batch_request_config(batch_request_config);

        let (local_addr, server_handle) = if is_http {
            // HTTP-specific settings
            let server = server_builder
                .http_only()
                .build(addr)
                .await
                .context("Failed building HTTP JSON-RPC server")?;
            (server.local_addr(), server.start(rpc))
        } else {
            // WS specific settings
            // TODO: websocket_requests_per_minute_limit read from config
            let server = server_builder
                .set_rpc_middleware(
                    RpcServiceBuilder::new()
                        .layer_fn(move |a| LimitMiddleware::new(a, NonZeroU32::new(5))),
                )
                .set_id_provider(EthSubscriptionIdProvider)
                .build(addr)
                .await
                .context("Failed building WS JSON-RPC server")?;
            (server.local_addr(), server.start(rpc))
        };
        let local_addr = local_addr.with_context(|| {
            format!("Failed getting local address for {transport_str} JSON-RPC server")
        })?;
        tracing::info!("Initialized {transport_str} API on {local_addr:?}");
        local_addr_sender.send(local_addr).ok();

        let close_handle = server_handle.clone();
        let vm_barrier = self
            .vm_barrier
            .clone()
            .expect("vm_barrier is not specified");
        let closing_vm_barrier = vm_barrier.clone();
        tokio::spawn(async move {
            if stop_receiver.changed().await.is_err() {
                tracing::warn!(
                    "Stop signal sender for {transport_str} JSON-RPC server was dropped \
                     without sending a signal"
                );
            }
            tracing::info!(
                "Stop signal received, {transport_str} JSON-RPC server is shutting down"
            );
            closing_vm_barrier.close();
            close_handle.stop().ok();
        });
        health_updater.update(HealthStatus::Ready.into());

        server_handle.stopped().await;
        drop(health_updater);
        tracing::info!("{transport_str} JSON-RPC server stopped");
        Self::wait_for_vm(vm_barrier, transport_str).await;
        Ok(())
    }

    async fn run_rpc_server(
        is_http: bool,
        rpc: RpcModule<()>,
        addr: SocketAddr,
        mut stop_receiver: watch::Receiver<bool>,
        health_updater: HealthUpdater,
        vm_barrier: VmConcurrencyBarrier,
        batch_request_config: BatchRequestConfig,
        response_body_size_limit: u32,
    ) {
        let transport = if is_http { "HTTP" } else { "WS" };
        let cors = is_http.then(|| {
            CorsLayer::new()
                .allow_methods([hyper::Method::POST])
                .allow_origin(tower_http::cors::Any)
                .allow_headers([hyper::header::CONTENT_TYPE])
        });
        let (in_flight_requests, counter) = InFlightRequestsLayer::pair();
        tokio::spawn(counter.run_emitter(Duration::from_secs(10), move |count| {
            metrics::histogram!("api.web3.in_flight_requests", count as f64, "scheme" => transport);
            future::ready(())
        }));
        let middleware = tower::ServiceBuilder::new()
            .layer(in_flight_requests)
            .option_layer(cors);

        let server_builder = if is_http {
            ServerBuilder::default().http_only().max_connections(5000)
        } else {
            ServerBuilder::default().ws_only()
        };

        let server = server_builder
            .set_batch_request_config(batch_request_config)
            .set_http_middleware(middleware)
            .max_response_body_size(response_body_size_limit)
            .build(addr)
            .await
            .unwrap_or_else(|err| {
                panic!("Failed building {} rpc server: {}", transport, err);
            });
        let server_handle = server.start(rpc);
        let close_handle = server_handle.clone();
        let close_vm_barrier = vm_barrier.clone();
        tokio::spawn(async move {
            if stop_receiver.changed().await.is_ok() {
                close_vm_barrier.close();
                close_handle.stop().ok();
            }
        });
        health_updater.update(HealthStatus::Ready.into());

        server_handle.stopped().await;
        drop(health_updater);
        olaos_logs::info!("{transport} JSON-RPC server stopped");
        Self::wait_for_vm(vm_barrier, transport).await;
    }

    async fn build_rpc_module(&self) -> RpcModule<()> {
        let rpc_app = self.build_rpc_state();
        let namespaces = self.namespaces.as_ref().unwrap();
        let mut rpc = RpcModule::new(());

        if namespaces.contains(&Namespace::Ola) {
            rpc.merge(OlaNamespace::new(rpc_app.clone()).into_rpc())
                .expect("Can't merge ola namespace");
        }
        if namespaces.contains(&Namespace::Eth) {
            rpc.merge(EthNamespace::new(rpc_app.clone()).into_rpc())
                .expect("Can't merge eth namespace");
        }

        rpc
    }

    async fn build_rpc_module_new(
        &self,
        pub_sub: Option<EthSubscribe>,
    ) -> anyhow::Result<RpcModule<()>> {
        let namespaces = self
            .namespaces
            .as_ref()
            .expect("namespaces not specified")
            .clone();
        let rpc_state = self.build_rpc_state();

        let mut rpc = RpcModule::new(());
        if let Some(pub_sub) = pub_sub {
            rpc.merge(pub_sub.into_rpc())
                .expect("Can't merge eth pubsub namespace");
        }
        if namespaces.contains(&Namespace::Ola) {
            rpc.merge(OlaNamespace::new(rpc_state.clone()).into_rpc())
                .expect("Can't merge ola namespace");
        }
        if namespaces.contains(&Namespace::Eth) {
            rpc.merge(EthNamespace::new(rpc_state.clone()).into_rpc())
                .expect("Can't merge eth namespace");
        }

        Ok(rpc)
    }

    fn build_rpc_state(&self) -> RpcState {
        let mut storage = self.pool.access_storage_tagged("api");
        let start_info = BlockStartInfo::new(&mut storage);

        drop(storage);
        RpcState {
            api_config: self.config.clone(),
            connection_pool: self.pool.clone(),
            tx_sender: self.tx_sender.clone(),
            start_info,
        }
    }

    async fn wait_for_vm(vm_barrier: VmConcurrencyBarrier, _transport: &str) {
        let wait_for_vm =
            tokio::time::timeout(SERVER_SHUTDOWN_TIMEOUT, vm_barrier.wait_until_stopped());
        let _ = wait_for_vm.await;
    }

    // fn into_full_params(self) -> anyhow::Result<FullApiParams> {
    //     Ok(FullApiParams {
    //         pool: self.pool,
    //         // last_miniblock_pool: self.last_miniblock_pool,
    //         config: self.config,
    //         transport: self.transport.context("API transport not set")?,
    //         // tx_sender: self.tx_sender.context("Transaction sender not set")?,
    //         vm_barrier: self.vm_barrier.context("VM barrier not set")?,
    //         polling_interval: self.polling_interval,
    //         namespaces: self.namespaces.unwrap_or_else(|| {
    //             olaos_logs::info!(
    //                 "debug_ and snapshots_ API namespace will be disabled by default in ApiBuilder"
    //             );
    //             Namespace::HTTP.to_vec()
    //         }),
    //         // optional: self.optional,
    //     })
    // }
}

async fn resolve_block(
    connection: &mut StorageProcessor<'_>,
    block: BlockId,
    method_name: &'static str,
) -> Result<MiniblockNumber, Web3Error> {
    let result = connection.blocks_web3_dal().resolve_block_id(block).await;
    result
        .map_err(|err| internal_error(method_name, err))?
        .ok_or(Web3Error::NoBlock)
}
