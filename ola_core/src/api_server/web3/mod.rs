use std::{net::SocketAddr, time::Duration};

use futures::future;
use jsonrpsee::{
    server::{BatchRequestConfig, ServerBuilder},
    RpcModule,
};
use ola_dal::connection::ConnectionPool;
use ola_web3_decl::namespaces::ola::OlaNamespaceServer;
use olaos_health_check::{HealthStatus, HealthUpdater, ReactiveHealthCheck};
use serde::Deserialize;
use tokio::sync::watch;
use tower_http::{cors::CorsLayer, metrics::InFlightRequestsLayer};

use self::{
    namespaces::ola::OlaNamespace,
    state::{InternalApiconfig, RpcState},
};

use super::{execution_sandbox::VmConcurrencyBarrier, tx_sender::TxSender};

pub mod backend;
pub mod namespaces;
pub mod state;

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
}

impl Namespace {
    pub const ALL: &'static [Namespace] = &[Namespace::Ola];
}

#[derive(Debug)]
pub struct ApiBuilder {
    transport: Option<ApiTransport>,
    subscriptions_limit: Option<usize>,
    config: InternalApiconfig,
    namespaces: Option<Vec<Namespace>>,
    threads: Option<usize>,
    tx_sender: Option<TxSender>,
    vm_barrier: Option<VmConcurrencyBarrier>,
    batch_request_size_limit: Option<usize>,
    response_body_size_limit: Option<usize>,
    filters_limit: Option<usize>,
    pool: ConnectionPool,
}

impl ApiBuilder {
    pub fn new(config: InternalApiconfig, pool: ConnectionPool) -> Self {
        Self {
            transport: None,
            subscriptions_limit: None,
            config,
            namespaces: None,
            threads: None,
            tx_sender: None,
            vm_barrier: None,
            batch_request_size_limit: None,
            response_body_size_limit: None,
            filters_limit: None,
            pool,
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

    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = Some(threads);
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

    pub fn enable_api_namespaces(mut self, namespaces: Vec<Namespace>) -> Self {
        self.namespaces = Some(namespaces);
        self
    }
}

impl ApiBuilder {
    pub async fn build(
        mut self,
        stop_receiver: watch::Receiver<bool>,
    ) -> (Vec<tokio::task::JoinHandle<()>>, ReactiveHealthCheck) {
        match self.transport.take() {
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

    async fn build_http(
        self,
        addr: SocketAddr,
        stop_receiver: watch::Receiver<bool>,
        health_updater: HealthUpdater,
    ) -> tokio::task::JoinHandle<()> {
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
        })
    }

    async fn build_ws(
        self,
        addr: SocketAddr,
        stop_receiver: watch::Receiver<bool>,
        health_updater: HealthUpdater,
    ) -> tokio::task::JoinHandle<()> {
        let rpc = self.build_rpc_module().await;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("ola-rpc-ws-worder")
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
        })
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
            .set_middleware(middleware)
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
        let ola_network_id = self.config.l2_chain_id;
        let rpc_app = self.build_rpc_state();
        let namespaces = self.namespaces.as_ref().unwrap();
        let mut rpc = RpcModule::new(());
        if namespaces.contains(&Namespace::Ola) {
            rpc.merge(OlaNamespace::new(rpc_app.clone()).into_rpc())
                .expect("Can't merge ola namespace");
        }
        rpc
    }

    fn build_rpc_state(&self) -> RpcState {
        RpcState {
            api_config: self.config.clone(),
            connection_pool: self.pool.clone(),
            tx_sender: self.tx_sender.clone().expect("failed to clone tx_sender"),
        }
    }

    async fn wait_for_vm(vm_barrier: VmConcurrencyBarrier, transport: &str) {
        let wait_for_vm =
            tokio::time::timeout(SERVER_SHUTDOWN_TIMEOUT, vm_barrier.wait_until_stopped());
        wait_for_vm.await;
    }
}
