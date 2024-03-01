use std::time::Duration;

use ola_config::{
    contracts::load_contracts_config, eth_sender::load_eth_sender_config,
    sequencer::load_network_config,
};
use ola_core::{
    genesis_init, initialize_components, is_genesis_needed, setup_sigint_handler, Component,
};
use ola_utils::wait_for_tasks::wait_for_tasks;
use olaos_logs::telemetry::{get_subscriber, init_subscriber, set_panic_hook};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (subscriber, _guard) = get_subscriber("olaos".into(), "info".into());
    init_subscriber(subscriber);
    set_panic_hook();
    olaos_logs::info!("init_subscriber finished");

    if is_genesis_needed().await {
        let eth_sender = load_eth_sender_config().expect("failed to load eth sender config");
        let network = load_network_config().expect("failed to load network config");
        let contracts = load_contracts_config().expect("failed to laod contract config");
        genesis_init(&eth_sender, &network, &contracts).await;
        olaos_logs::info!("genesis_init finished");
    }

    let components = vec![
        Component::HttpApi,
        Component::Sequencer,
        Component::Tree,
        Component::OffChainVerifier,
    ];
    let (core_task_handles, stop_sender, health_check_handle) = initialize_components(components)
        .await
        .expect("Unable to start Core actors");

    olaos_logs::info!("Running {} core task handlers", core_task_handles.len());
    let sigint_receiver = setup_sigint_handler();

    let graceful_shutdown = None::<futures::future::Ready<()>>;
    tokio::select! {
        _ = wait_for_tasks(core_task_handles, graceful_shutdown, false) => {},
        _ = sigint_receiver => {
            olaos_logs::info!("Stop signal received, shutting down");
        },
    }
    stop_sender.send(true).ok();
    tokio::time::sleep(Duration::from_secs(5)).await;
    health_check_handle.stop().await;
    olaos_logs::info!("Stopped");
    Ok(())
}
