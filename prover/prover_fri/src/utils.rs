use std::time::Instant;

use ola_dal::StorageProcessor;
use ola_types::L1BatchNumber;
use olaos_object_store::ObjectStore;
use olaos_prover_fri_types::{verifier, AllProof, CircuitWrapper, FriProofWrapper, C, D, F};

pub struct ProverArtifacts {
    block_number: L1BatchNumber,
    pub proof_wrapper: FriProofWrapper,
}

impl ProverArtifacts {
    pub fn new(block_number: L1BatchNumber, proof_wrapper: FriProofWrapper) -> Self {
        Self {
            block_number,
            proof_wrapper,
        }
    }
}

pub async fn save_proof(
    job_id: u32,
    started_at: Instant,
    artifacts: ProverArtifacts,
    blob_store: &dyn ObjectStore,
    _public_blob_store: Option<&dyn ObjectStore>,
    _shall_save_to_public_bucket: bool,
    storage_processor: &mut StorageProcessor<'_>,
) {
    olaos_logs::info!(
        "Successfully proven job: {}, total time taken: {:?}",
        job_id,
        started_at.elapsed()
    );
    let blob_save_started_at = Instant::now();
    let proof = artifacts.proof_wrapper;
    let blob_url = blob_store
        .put(artifacts.block_number, &proof)
        .await
        .unwrap();

    olaos_logs::info!("blob_save_time {:?}", blob_save_started_at.elapsed());

    let mut transaction = storage_processor.start_transaction().await;
    let _job_metadata = transaction
        .fri_prover_jobs_dal()
        .save_proof(job_id, started_at.elapsed(), &blob_url)
        .await;
    transaction.commit().await;
}

pub fn verify_proof(circuit_wrapper: CircuitWrapper, proof: AllProof<F, C, D>, job_id: u32) {
    let is_valid = match circuit_wrapper {
        CircuitWrapper::Base(base_circuit) => {
            verifier::verify_proof::<F, C, D>(base_circuit.ola_stark, proof, &base_circuit.config)
                .is_ok()
        }
    };
    if !is_valid {
        let msg = format!("Failed to verify base layer proof for job-id: {job_id}");
        olaos_logs::error!("{}", msg);
        panic!("{}", msg);
    }
}
