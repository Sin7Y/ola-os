use std::time::Instant;

use ola_types::L1BatchNumber;
use olaos_prover_fri_types::{AllProof, CircuitWrapper, FriProofWrapper, C, D, F};

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

// pub async fn save_proof(
//     job_id: u32,
//     started_at: Instant,
//     artifacts: ProverArtifacts,
//     blob_store: &dyn ObjectStore,
//     public_blob_store: Option<&dyn ObjectStore>,
//     shall_save_to_public_bucket: bool,
//     storage_processor: &mut StorageProcessor<'_>,
// ) {
//     tracing::info!(
//         "Successfully proven job: {}, total time taken: {:?}",
//         job_id,
//         started_at.elapsed()
//     );
//     let proof = artifacts.proof_wrapper;

//     // We save the scheduler proofs in public bucket,
//     // so that it can be verified independently while we're doing shadow proving
//     let (circuit_type, is_scheduler_proof) = match &proof {
//         FriProofWrapper::Base(base) => (base.numeric_circuit_type(), false),
//         FriProofWrapper::Recursive(recursive_circuit) => match recursive_circuit {
//             ZkSyncRecursionLayerProof::SchedulerCircuit(_) => {
//                 if shall_save_to_public_bucket {
//                     public_blob_store
//                         .expect("public_object_store shall not be empty while running with shall_save_to_public_bucket config")
//                         .put(artifacts.block_number.0, &proof)
//                         .await
//                         .unwrap();
//                 }
//                 (recursive_circuit.numeric_circuit_type(), true)
//             }
//             _ => (recursive_circuit.numeric_circuit_type(), false),
//         },
//     };

//     let blob_save_started_at = Instant::now();
//     let blob_url = blob_store.put(job_id, &proof).await.unwrap();

//     METRICS.blob_save_time[&circuit_type.to_string()].observe(blob_save_started_at.elapsed());

//     let mut transaction = storage_processor.start_transaction().await.unwrap();
//     let job_metadata = transaction
//         .fri_prover_jobs_dal()
//         .save_proof(job_id, started_at.elapsed(), &blob_url)
//         .await;
//     if is_scheduler_proof {
//         transaction
//             .fri_proof_compressor_dal()
//             .insert_proof_compression_job(artifacts.block_number, &blob_url)
//             .await;
//     }
//     if job_metadata.is_node_final_proof {
//         transaction
//             .fri_scheduler_dependency_tracker_dal()
//             .set_final_prover_job_id_for_l1_batch(
//                 get_base_layer_circuit_id_for_recursive_layer(job_metadata.circuit_id),
//                 job_id,
//                 job_metadata.block_number,
//             )
//             .await;
//     }
//     transaction.commit().await.unwrap();
// }

pub fn verify_proof(circuit_wrapper: &CircuitWrapper, proof: &AllProof<F, C, D>, job_id: u32) {}
