use anyhow::Ok;
use ola_config::{
    fri_witness_generator::load_fri_witness_generator_config,
    object_store::load_object_store_config,
};
use ola_dal::connection::{ConnectionPool, DbVariant};
use olaos_logs::telemetry::{get_subscriber, init_subscriber};
use olaos_object_store::ObjectStoreFactory;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "Run witness generator for different aggregation round",
    about = "Component for generating witness"
)]
struct Opt {
    /// Number of times witness generator should be run.
    #[structopt(short = "b", long = "batch_size")]
    batch_size: Option<usize>,
    /// Aggregation rounds options, they can be run individually or together.
    ///
    /// Single aggregation round for the witness generator.
    #[structopt(short = "r", long = "round")]
    round: Option<AggregationRound>,
    /// Start all aggregation rounds for the witness generator.
    #[structopt(short = "a", long = "all_rounds")]
    all_rounds: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (subscriber, _guard) = get_subscriber("olaos_prover_fri_gateway".into(), "info".into());
    init_subscriber(subscriber);
    olaos_logs::info!("init_subscriber finished");

    let opt = Opt::from_args();
    let config =
        load_fri_witness_generator_config().expect("failed to load fri witness generator config");
    let pool = ConnectionPool::builder(DbVariant::Master).build().await;
    let prover_connection_pool = ConnectionPool::builder(DbVariant::Prover).build().await;
    let object_store_config =
        load_object_store_config().expect("failed to load object store config");
    let store_factory = ObjectStoreFactory::new(object_store_config);
    let (stop_sender, stop_receiver) = watch::channel(false);

    let protocol_versions = prover_connection_pool
        .access_storage()
        .await
        .unwrap()
        .fri_protocol_versions_dal()
        .protocol_versions()
        .await;

    // If batch_size is none, it means that the job is 'looping forever' (this is the usual setup in local network).
    // At the same time, we're reading the protocol_version only once at startup - so if there is no protocol version
    // read (this is often due to the fact, that the gateway was started too late, and it didn't put the updated protocol
    // versions into the database) - then the job will simply 'hang forever' and not pick any tasks.
    if opt.batch_size.is_none() && protocol_versions.is_empty() {
        panic!(
            "Could not find a protocol version for my commitments. Is gateway running?  Maybe you started this job before gateway updated the database?",
        );
    }

    let rounds = match (opt.round, opt.all_rounds) {
        (Some(round), false) => vec![round],
        (None, true) => vec![AggregationRound::BasicCircuits],
        (Some(_), true) => {
            return Err(anyhow!(
                "Cannot set both the --all_rounds and --round flags. Choose one or the other."
            ));
        }
        (None, false) => {
            return Err(anyhow!(
                "Expected --all_rounds flag with no --round flag present"
            ));
        }
    };
    let mut tasks = Vec::new();

    for (i, round) in rounds.iter().enumerate() {
        olaos_logs::info!(
            "initializing the {:?} witness generator, batch size: {:?} with protocol_versions: {:?}",
            round,
            opt.batch_size,
            &protocol_versions
        );

        let witness_generator_task = match round {
            AggregationRound::BasicCircuits => {
                let public_blob_store = match config.shall_save_to_public_bucket {
                    false => None,
                    true => Some(
                        ObjectStoreFactory::new(load_object_store_config().unwrap())
                            .create_store()
                            .await,
                    ),
                };
                let generator = BasicWitnessGenerator::new(
                    config.clone(),
                    &store_factory,
                    public_blob_store,
                    connection_pool.clone(),
                    prover_connection_pool.clone(),
                    protocol_versions.clone(),
                )
                .await;
                generator.run(stop_receiver.clone(), opt.batch_size)
            }
        };

        tasks.push(tokio::spawn(witness_generator_task));

        olaos_logs::info!(
            "initialized {:?} witness generator in {:?}",
            round,
            started_at.elapsed()
        );
    }

    let (mut stop_signal_sender, mut stop_signal_receiver) = mpsc::channel(256);
    ctrlc::set_handler(move || {
        block_on(stop_signal_sender.send(true)).expect("Ctrl+C signal send");
    })
    .expect("Error setting Ctrl+C handler");
    let graceful_shutdown = None::<futures::future::Ready<()>>;
    let tasks_allowed_to_finish = true;
    tokio::select! {
        _ = wait_for_tasks(tasks, None, graceful_shutdown, tasks_allowed_to_finish) => {},
        _ = stop_signal_receiver.next() => {
            olaos_logs::info!("Stop signal received, shutting down");
        }
    }

    stop_sender.send(true).ok();
    olaos_logs::info!("Finished witness generation");
    Ok(())
}
