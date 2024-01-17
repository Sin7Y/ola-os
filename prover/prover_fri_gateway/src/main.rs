use crate::api_data_fetcher::{PeriodicApiStruct, PROOF_GENERATION_DATA_PATH, SUBMIT_PROOF_PATH};
use anyhow::Context as _;
use ola_config::{
    fri_prover_gateway::load_prover_fri_gateway_config, object_store::load_object_store_config,
};
use ola_dal::connection::{ConnectionPool, DbVariant};
use ola_types::prover_server_api::{ProofGenerationDataRequest, SubmitProofRequest};
use ola_utils::wait_for_tasks::wait_for_tasks;
use olaos_logs::telemetry::{get_subscriber, init_subscriber};
use olaos_object_store::ObjectStoreFactory;
use reqwest::Client;
use tokio::sync::{oneshot, watch};

mod api_data_fetcher;
mod proof_gen_data_fetcher;
mod proof_submitter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (subscriber, _guard) = get_subscriber("olaos_prover_fri_gateway".into(), "info".into());
    init_subscriber(subscriber);
    olaos_logs::info!("init_subscriber finished");

    let config =
        load_prover_fri_gateway_config().expect("failed to load prover fri gateway config");
    let pool = ConnectionPool::builder(DbVariant::Prover).build().await;
    let object_store_config =
        load_object_store_config().expect("failed to load object store config");
    let store_factory = ObjectStoreFactory::new(object_store_config);

    let proof_submitter = PeriodicApiStruct {
        blob_store: store_factory.create_store().await,
        pool: pool.clone(),
        api_url: format!("{}{SUBMIT_PROOF_PATH}", config.api_url),
        poll_duration: config.api_poll_duration(),
        client: Client::new(),
    };
    let proof_gen_data_fetcher = PeriodicApiStruct {
        blob_store: store_factory.create_store().await,
        pool,
        api_url: format!("{}{PROOF_GENERATION_DATA_PATH}", config.api_url),
        poll_duration: config.api_poll_duration(),
        client: Client::new(),
    };

    let (stop_sender, stop_receiver) = watch::channel(false);

    let (stop_signal_sender, stop_signal_receiver) = oneshot::channel();
    let mut stop_signal_sender = Some(stop_signal_sender);
    ctrlc::set_handler(move || {
        if let Some(stop_signal_sender) = stop_signal_sender.take() {
            stop_signal_sender.send(()).ok();
        }
    })
    .context("Error setting Ctrl+C handler")?;

    olaos_logs::info!("Starting Fri Prover Gateway");

    let tasks = vec![
        tokio::spawn(
            proof_gen_data_fetcher.run::<ProofGenerationDataRequest>(stop_receiver.clone()),
        ),
        tokio::spawn(proof_submitter.run::<SubmitProofRequest>(stop_receiver)),
    ];

    let graceful_shutdown = None::<futures::future::Ready<()>>;
    let tasks_allowed_to_finish = false;
    tokio::select! {
        _ = wait_for_tasks(tasks, graceful_shutdown, tasks_allowed_to_finish) => {},
        _ = stop_signal_receiver => {
            olaos_logs::info!("Stop signal received, shutting down");
        }
    };
    stop_sender.send(true).ok();

    Ok(())
}
