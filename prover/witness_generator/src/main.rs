use anyhow::Ok;
use ola_config::{
    fri_witness_generator::load_fri_witness_generator_config,
    object_store::load_object_store_config,
};
use ola_dal::connection::{ConnectionPool, DbVariant};
use olaos_logs::telemetry::{get_subscriber, init_subscriber};
use olaos_object_store::ObjectStoreFactory;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (subscriber, _guard) = get_subscriber("olaos_prover_fri_gateway".into(), "info".into());
    init_subscriber(subscriber);
    olaos_logs::info!("init_subscriber finished");

    let config =
        load_fri_witness_generator_config().expect("failed to load fri witness generator config");
    let pool = ConnectionPool::builder(DbVariant::Master).build().await;
    let object_store_config =
        load_object_store_config().expect("failed to load object store config");
    let store_factory = ObjectStoreFactory::new(object_store_config);

    Ok(())
}
