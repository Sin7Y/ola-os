use std::{collections::HashSet, net::SocketAddr, sync::Arc, time::Duration};

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use olaos_health_check::{AppHealth, CheckHealth};
use tokio::sync::watch;

type SharedHealthchecks = Arc<[Box<dyn CheckHealth>]>;

async fn check_health(health_checks: State<SharedHealthchecks>) -> (StatusCode, Json<AppHealth>) {
    let response = AppHealth::new(&health_checks).await;
    let response_code = if response.is_ready() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (response_code, Json(response))
}

async fn run_server(
    bind_address: &SocketAddr,
    health_checks: Vec<Box<dyn CheckHealth>>,
    mut stop_receiver: watch::Receiver<bool>,
) {
    let mut health_check_names = HashSet::with_capacity(health_checks.len());
    for check in &health_checks {
        let health_check_name = check.name();
        health_check_names.insert(health_check_name);
    }

    let health_checks = SharedHealthchecks::from(health_checks);
    let app = Router::new()
        .route("/health", get(check_health))
        .with_state(health_checks);

    axum::Server::bind(bind_address)
        .serve(app.into_make_service())
        .with_graceful_shutdown(async move {
            if stop_receiver.changed().await.is_err() {
                olaos_logs::error!("Stop signal sender for healthcheck server was dropped without sending a signal");
            }
            olaos_logs::info!("Stop signal received, healthcheck server is shutting down");
        })
        .await
        .expect("Healthcheck server failed");
    olaos_logs::info!("Healthcheck server shut down");
}

#[derive(Debug)]
pub struct HealthCheckHandle {
    server: tokio::task::JoinHandle<()>,
    stop_sender: watch::Sender<bool>,
}

impl HealthCheckHandle {
    pub fn spawn_server(addr: SocketAddr, healthchecks: Vec<Box<dyn CheckHealth>>) -> Self {
        let (stop_sender, stop_receiver) = watch::channel(false);
        let server = tokio::spawn(async move {
            run_server(&addr, healthchecks, stop_receiver).await;
        });

        Self {
            server,
            stop_sender,
        }
    }

    pub async fn stop(self) {
        // Paradoxically, `hyper` server is quite slow to shut down if it isn't queried during shutdown:
        // https://github.com/hyperium/hyper/issues/3188. It is thus recommended to set a timeout for shutdown.
        const GRACEFUL_SHUTDOWN_WAIT: Duration = Duration::from_secs(10);

        self.stop_sender.send(true).ok();
        let server_result = tokio::time::timeout(GRACEFUL_SHUTDOWN_WAIT, self.server).await;
        if let Ok(server_result) = server_result {
            // Propagate potential panics from the server task.
            server_result.unwrap();
        } else {
            println!("Timed out {GRACEFUL_SHUTDOWN_WAIT:?} waiting for healthcheck server to gracefully shut down");
        }
    }
}
