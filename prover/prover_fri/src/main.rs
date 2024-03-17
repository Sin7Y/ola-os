use std::sync::Arc;

use anyhow::Context;
use ola_config::{
    fri_prover::{load_prover_fri_config, FriProverConfig},
    object_store::{load_prover_object_store_config, load_public_object_store_config},
};
use ola_dal::connection::{ConnectionPool, DbVariant};
use ola_utils::wait_for_tasks::wait_for_tasks;
use olaos_logs::telemetry::{get_subscriber, init_subscriber};
use olaos_object_store::{ObjectStore, ObjectStoreFactory};
use olaos_prover_fri::prover_job_processor::Prover;
use olaos_queued_job_processor::JobProcessor;
use tokio::{
    sync::{oneshot, watch::Receiver},
    task::JoinHandle,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (subscriber, _guard) = get_subscriber("olaos_prover_fri".into(), "info".into());
    init_subscriber(subscriber);
    olaos_logs::info!("init_subscriber finished");

    let prover_config = load_prover_fri_config().expect("failed to load fri prover config");
    let (stop_signal_sender, stop_signal_receiver) = oneshot::channel();
    let mut stop_signal_sender = Some(stop_signal_sender);
    ctrlc::set_handler(move || {
        if let Some(sender) = stop_signal_sender.take() {
            sender.send(()).ok();
        }
    })
    .context("Error setting Ctrl+C handler")?;

    let (stop_sender, stop_receiver) = tokio::sync::watch::channel(false);

    let object_store_config =
        load_prover_object_store_config().expect("failed to load prover object store config");
    let object_store_factory = ObjectStoreFactory::new(object_store_config.0);
    let public_object_store_config =
        load_public_object_store_config().expect("failed to load public object store config");
    let public_blob_store = match prover_config.shall_save_to_public_bucket {
        false => None,
        true => Some(
            ObjectStoreFactory::new(public_object_store_config.0)
                .create_store()
                .await,
        ),
    };
    olaos_logs::info!("Starting FRI proof generation");

    // There are 2 threads using the connection pool:
    // 1. The prover thread, which is used to update the prover job status.
    // 2. The socket listener thread, which is used to update the prover instance status.

    let pool = ConnectionPool::builder(DbVariant::Prover)
        .set_max_size(Some(2))
        .build()
        .await;
    let port = prover_config.witness_vector_receiver_port;
    let prover_tasks = get_prover_tasks(
        prover_config,
        stop_receiver.clone(),
        object_store_factory,
        public_blob_store,
        pool,
        // circuit_ids_for_round_to_be_proven,
    )
    .await
    .context("get_prover_tasks()")?;

    let mut tasks = vec![];
    tasks.extend(prover_tasks);

    let tasks_allowed_to_finish = false;
    let graceful_shutdown = None::<futures::future::Ready<()>>;
    tokio::select! {
        _ = wait_for_tasks(tasks, graceful_shutdown, tasks_allowed_to_finish) => {},
        _ = stop_signal_receiver => {
            olaos_logs::info!("Stop signal received, shutting down");
        },
    }

    stop_sender.send(true).ok();
    Ok(())
}

async fn get_prover_tasks(
    prover_config: FriProverConfig,
    stop_receiver: Receiver<bool>,
    store_factory: ObjectStoreFactory,
    public_blob_store: Option<Arc<dyn ObjectStore>>,
    pool: ConnectionPool,
    //     // circuit_ids_for_round_to_be_proven: Vec<CircuitIdRoundTuple>,
) -> anyhow::Result<Vec<JoinHandle<anyhow::Result<()>>>> {
    //     // use zksync_vk_setup_data_server_fri::commitment_utils::get_cached_commitments;

    //     // use crate::prover_job_processor::{load_setup_data_cache, Prover};

    //     // let vk_commitments = get_cached_commitments();

    //     // olaos_logs::info!(
    //     //     "Starting CPU FRI proof generation for with vk_commitments: {:?}",
    //     //     vk_commitments
    //     // );

    //     // let setup_load_mode =
    //     //     load_setup_data_cache(&prover_config).context("load_setup_data_cache()")?;
    let prover = Prover::new(
        store_factory.create_store().await,
        public_blob_store,
        prover_config,
        pool,
        // setup_load_mode,
        // circuit_ids_for_round_to_be_proven,
        // vk_commitments,
    );
    Ok(vec![tokio::spawn(prover.run(stop_receiver, None))])
}
