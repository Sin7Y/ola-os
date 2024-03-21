use std::{sync::Arc, time::Instant};

use anyhow::Context;
use async_trait::async_trait;
use ola_config::fri_prover::FriProverConfig;
use ola_dal::connection::ConnectionPool;
use ola_types::basic_fri_types::CircuitIdRoundTuple;
use olaos_object_store::ObjectStore;
use olaos_prover_fri_types::{
    circuits::OlaBaseLayerCircuit, prove_with_traces, CircuitWrapper, FriProofWrapper,
    OlaBaseLayerProof, ProverJob, TimingTree, C, D, F,
};
use olaos_prover_fri_utils::fetch_next_circuit;
use olaos_queued_job_processor::JobProcessor;
use tokio::task::JoinHandle;

use crate::utils::{save_proof, verify_proof, ProverArtifacts};

pub struct Prover {
    blob_store: Arc<dyn ObjectStore>,
    public_blob_store: Option<Arc<dyn ObjectStore>>,
    config: Arc<FriProverConfig>,
    prover_connection_pool: ConnectionPool,
    // Only pick jobs for the configured circuit id and aggregation rounds.
    // Empty means all jobs are picked.
    circuit_ids_for_round_to_be_proven: Vec<CircuitIdRoundTuple>,
}

impl Prover {
    pub fn new(
        blob_store: Arc<dyn ObjectStore>,
        public_blob_store: Option<Arc<dyn ObjectStore>>,
        config: FriProverConfig,
        prover_connection_pool: ConnectionPool,
        circuit_ids_for_round_to_be_proven: Vec<CircuitIdRoundTuple>,
    ) -> Self {
        Prover {
            blob_store,
            public_blob_store,
            config: Arc::new(config),
            prover_connection_pool,
            circuit_ids_for_round_to_be_proven,
        }
    }

    pub fn prove(
        job: ProverJob,
        config: Arc<FriProverConfig>,
        // setup_data: Arc<GoldilocksProverSetupData>,
    ) -> ProverArtifacts {
        let proof = match job.circuit_wrapper {
            CircuitWrapper::Base(base_circuit) => {
                Self::prove_base_layer(job.job_id, base_circuit, config)
            }
        };
        ProverArtifacts::new(job.block_number, proof)
    }

    fn prove_base_layer(
        job_id: u32,
        circuit: OlaBaseLayerCircuit,
        _config: Arc<FriProverConfig>,
        // artifact: Arc<GoldilocksProverSetupData>,
    ) -> FriProofWrapper {
        let start = Instant::now();
        // TODO: Cloning is not great, having prove_with_traces accept a reference
        let proof = prove_with_traces::<F, C, D>(
            &circuit.ola_stark,
            &circuit.config,
            circuit.witness.clone(),
            circuit.public_values.clone(),
            &mut TimingTree::default(),
        )
        .unwrap();

        olaos_logs::info!("proof_generation_time {:?}", start.elapsed());

        verify_proof(CircuitWrapper::Base(circuit.clone()), proof.clone(), job_id);
        FriProofWrapper::Base(OlaBaseLayerProof {
            ola_stark: circuit.ola_stark,
            proof,
            config: circuit.config,
        })
    }
}

#[async_trait]
impl JobProcessor for Prover {
    type Job = ProverJob;
    type JobId = u32;
    type JobArtifacts = ProverArtifacts;
    const SERVICE_NAME: &'static str = "OlaFriCpuProver";

    async fn get_next_job(&self) -> anyhow::Result<Option<(Self::JobId, Self::Job)>> {
        let mut storage = self.prover_connection_pool.access_storage().await;
        let Some(prover_job) = fetch_next_circuit(
            &mut storage,
            &*self.blob_store,
            &self.circuit_ids_for_round_to_be_proven,
            // &self.vk_commitments,
        )
        .await
        else {
            return Ok(None);
        };
        Ok(Some((prover_job.job_id, prover_job)))
    }

    async fn save_failure(&self, job_id: Self::JobId, _started_at: Instant, error: String) {
        self.prover_connection_pool
            .access_storage()
            .await
            .fri_prover_jobs_dal()
            .save_proof_error(job_id, error)
            .await;
    }

    async fn process_job(
        &self,
        job: Self::Job,
        _started_at: Instant,
    ) -> JoinHandle<anyhow::Result<Self::JobArtifacts>> {
        let config = Arc::clone(&self.config);
        // let setup_data = self.get_setup_data(job.setup_data_key.clone());
        tokio::task::spawn_blocking(move || {
            Ok(Self::prove(
                job, config,
                // setup_data.context("get_setup_data()")?,
            ))
        })
    }

    async fn save_result(
        &self,
        job_id: Self::JobId,
        started_at: Instant,
        artifacts: Self::JobArtifacts,
    ) -> anyhow::Result<()> {
        olaos_logs::info!("cpu_total_proving_time {:?}", started_at.elapsed());

        let mut storage_processor = self.prover_connection_pool.access_storage().await;
        save_proof(
            job_id,
            started_at,
            artifacts,
            &*self.blob_store,
            self.public_blob_store.as_deref(),
            self.config.shall_save_to_public_bucket,
            &mut storage_processor,
        )
        .await;
        Ok(())
    }

    fn max_attempts(&self) -> u32 {
        self.config.max_attempts
    }

    async fn get_job_attempts(&self, job_id: &u32) -> anyhow::Result<u32> {
        let mut prover_storage = self.prover_connection_pool.access_storage().await;
        prover_storage
            .fri_prover_jobs_dal()
            .get_prover_job_attempts(*job_id)
            .await
            .map(|attempts| attempts.unwrap_or(0))
            .context("failed to get job attempts for Prover")
    }
}
