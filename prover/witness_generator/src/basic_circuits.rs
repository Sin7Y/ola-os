use std::{sync::Arc, time::Instant};

use async_trait::async_trait;
use ola_config::fri_witness_generator::FriWitnessGeneratorConfig;
use ola_dal::connection::ConnectionPool;
use ola_types::{
    proofs::PrepareBasicCircuitsJob, protocol_version::FriProtocolVersionId, L1BatchNumber,
};
use olaos_object_store::{ObjectStore, ObjectStoreFactory};
use olaos_queued_job_processor::JobProcessor;

#[derive(Clone)]
pub struct BasicWitnessGeneratorJob {
    block_number: L1BatchNumber,
    job: PrepareBasicCircuitsJob,
}

pub struct BasicCircuitArtifacts {
    // basic_circuits: BlockBasicCircuits<GoldilocksField, ZkSyncDefaultRoundFunction>,
    // basic_circuits_inputs: BlockBasicCircuitsPublicInputs<GoldilocksField>,
    // per_circuit_closed_form_inputs: BlockBasicCircuitsPublicCompactFormsWitnesses<GoldilocksField>,
    // #[allow(dead_code)]
    // scheduler_witness: SchedulerCircuitInstanceWitness<
    //     GoldilocksField,
    //     CircuitGoldilocksPoseidon2Sponge,
    //     GoldilocksExt2,
    // >,
    // aux_output_witness: BlockAuxilaryOutputWitness<GoldilocksField>,
}

#[derive(Debug)]
pub struct BasicWitnessGenerator {
    config: Arc<FriWitnessGeneratorConfig>,
    object_store: Arc<dyn ObjectStore>,
    public_blob_store: Option<Arc<dyn ObjectStore>>,
    connection_pool: ConnectionPool,
    prover_connection_pool: ConnectionPool,
    protocol_versions: Vec<FriProtocolVersionId>,
}

impl BasicWitnessGenerator {
    pub async fn new(
        config: FriWitnessGeneratorConfig,
        store_factory: &ObjectStoreFactory,
        public_blob_store: Option<Arc<dyn ObjectStore>>,
        connection_pool: ConnectionPool,
        prover_connection_pool: ConnectionPool,
        protocol_versions: Vec<FriProtocolVersionId>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            object_store: store_factory.create_store().await,
            public_blob_store,
            connection_pool,
            prover_connection_pool,
            protocol_versions,
        }
    }
}

#[async_trait]
impl JobProcessor for BasicWitnessGenerator {
    type Job = BasicWitnessGeneratorJob;
    type JobId = L1BatchNumber;
    // The artifact is optional to support skipping blocks when sampling is enabled.
    type JobArtifacts = Option<BasicCircuitArtifacts>;

    const SERVICE_NAME: &'static str = "fri_basic_circuit_witness_generator";

    async fn get_next_job(&self) -> anyhow::Result<Option<(Self::JobId, Self::Job)>> {
        let mut prover_connection = self.prover_connection_pool.access_storage().await.unwrap();
        let last_l1_batch_to_process = self.config.last_l1_batch_to_process();
        let pod_name = get_current_pod_name();
        match prover_connection
            .fri_witness_generator_dal()
            .get_next_basic_circuit_witness_job(
                last_l1_batch_to_process,
                &self.protocol_versions,
                &pod_name,
            )
            .await
        {
            Some(block_number) => {
                olaos_logs::info!(
                    "Processing FRI basic witness-gen for block {}",
                    block_number
                );
                let started_at = Instant::now();
                let job = get_artifacts(block_number, &*self.object_store).await;

                WITNESS_GENERATOR_METRICS.blob_fetch_time[&AggregationRound::BasicCircuits.into()]
                    .observe(started_at.elapsed());

                Ok(Some((block_number, job)))
            }
            None => Ok(None),
        }
    }

    async fn save_failure(&self, job_id: L1BatchNumber, _started_at: Instant, error: String) -> () {
        self.prover_connection_pool
            .access_storage()
            .await
            .fri_witness_generator_dal()
            .mark_witness_job_failed(&error, job_id)
            .await;
    }

    #[allow(clippy::async_yields_async)]
    async fn process_job(
        &self,
        job: BasicWitnessGeneratorJob,
        started_at: Instant,
    ) -> tokio::task::JoinHandle<anyhow::Result<Option<BasicCircuitArtifacts>>> {
        let config = Arc::clone(&self.config);
        let object_store = Arc::clone(&self.object_store);
        let connection_pool = self.connection_pool.clone();
        let prover_connection_pool = self.prover_connection_pool.clone();
        tokio::spawn(async move {
            Ok(Self::process_job_impl(
                object_store,
                connection_pool,
                prover_connection_pool,
                job,
                started_at,
                config,
            )
            .await)
        })
    }

    async fn save_result(
        &self,
        job_id: L1BatchNumber,
        started_at: Instant,
        optional_artifacts: Option<BasicCircuitArtifacts>,
    ) -> anyhow::Result<()> {
        match optional_artifacts {
            None => Ok(()),
            Some(artifacts) => {
                let blob_started_at = Instant::now();
                let blob_urls = save_artifacts(
                    job_id,
                    artifacts,
                    &*self.object_store,
                    self.public_blob_store.as_deref(),
                    self.config.shall_save_to_public_bucket,
                )
                .await;

                WITNESS_GENERATOR_METRICS.blob_save_time[&AggregationRound::BasicCircuits.into()]
                    .observe(blob_started_at.elapsed());

                update_database(&self.prover_connection_pool, started_at, job_id, blob_urls).await;
                Ok(())
            }
        }
    }

    fn max_attempts(&self) -> u32 {
        self.config.max_attempts
    }

    async fn get_job_attempts(&self, job_id: &L1BatchNumber) -> anyhow::Result<u32> {
        let mut prover_storage = self
            .prover_connection_pool
            .access_storage()
            .await
            .context("failed to acquire DB connection for BasicWitnessGenerator")?;
        prover_storage
            .fri_witness_generator_dal()
            .get_basic_circuit_witness_job_attempts(*job_id)
            .await
            .map(|attempts| attempts.unwrap_or(0))
            .context("failed to get job attempts for BasicWitnessGenerator")
    }
}
