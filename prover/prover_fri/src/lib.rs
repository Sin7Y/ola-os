use std::time::Instant;

use ola_dal::StorageProcessor;
use olaos_object_store::ObjectStore;
use olaos_prover_fri_types::{get_current_pod_name, ProverJob};

pub mod prover_job_processor;
pub mod utils;

// pub async fn fetch_next_circuit(
//     storage: &mut StorageProcessor<'_>,
//     blob_store: &dyn ObjectStore,
//     // circuit_ids_for_round_to_be_proven: &Vec<CircuitIdRoundTuple>,
//     // vk_commitments: &L1VerifierConfig,
// ) -> Option<ProverJob> {
//     // let protocol_versions = storage
//     //     .fri_protocol_versions_dal()
//     //     .protocol_version_for(vk_commitments)
//     //     .await;
//     let pod_name = get_current_pod_name();
//     let prover_job = match &circuit_ids_for_round_to_be_proven.is_empty() {
//         false => {
//             // Specialized prover: proving subset of configured circuits.
//             storage
//                 .fri_prover_jobs_dal()
//                 .get_next_job_for_circuit_id_round(
//                     circuit_ids_for_round_to_be_proven,
//                     &protocol_versions,
//                     &pod_name,
//                 )
//                 .await
//         }
//         true => {
//             // Generalized prover: proving all circuits.
//             storage
//                 .fri_prover_jobs_dal()
//                 .get_next_job(&protocol_versions, &pod_name)
//                 .await
//         }
//     }?;
//     olaos_logs::info!("Started processing prover job: {:?}", prover_job);

//     // let circuit_key = FriCircuitKey {
//     //     block_number: prover_job.block_number,
//     //     sequence_number: prover_job.sequence_number,
//     //     circuit_id: prover_job.circuit_id,
//     //     aggregation_round: prover_job.aggregation_round,
//     //     depth: prover_job.depth,
//     // };
//     let started_at = Instant::now();
//     // let input = blob_store
//     //     .get(circuit_key)
//     //     .await
//     //     .unwrap_or_else(|err| panic!("{err:?}"));

//     // let label = CircuitLabels {
//     //     circuit_type: prover_job.circuit_id,
//     //     aggregation_round: prover_job.aggregation_round.into(),
//     // };
//     // PROVER_FRI_UTILS_METRICS.blob_fetch_time[&label].observe(started_at.elapsed());

//     // let setup_data_key = ProverServiceDataKey {
//     //     circuit_id: prover_job.circuit_id,
//     //     round: prover_job.aggregation_round,
//     // };
//     Some(ProverJob::new(
//         prover_job.block_number,
//         prover_job.id,
//         // input,
//         // setup_data_key,
//     ))
// }
