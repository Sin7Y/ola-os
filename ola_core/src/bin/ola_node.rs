use std::time::Duration;

use ola_core::{initialize_components, setup_sigint_handler, Component};
use ola_utils::wait_for_tasks::wait_for_tasks;
use olaos_logs::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = get_subscriber("olaos".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let components = vec![Component::HttpApi];
    let (core_task_handles, stop_sender, health_check_handle) = initialize_components(components)
        .await
        .expect("Unable to start Core actors");
    let sigint_receiver = setup_sigint_handler();
    let graceful_shutdown = None::<futures::future::Ready<()>>;
    tokio::select! {
        _ = wait_for_tasks(core_task_handles, graceful_shutdown, false) => {},
        _ = sigint_receiver => {
            olaos_logs::info!("Stop signal received, shutting down");
        }
    }
    stop_sender.send(true).ok();
    tokio::time::sleep(Duration::from_secs(5)).await;
    health_check_handle.stop().await;
    olaos_logs::info!("Stopped");
    Ok(())
}
