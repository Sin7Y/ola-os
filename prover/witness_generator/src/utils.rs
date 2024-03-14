// pub async fn save_base_prover_input_artifacts(
//     block_number: L1BatchNumber,
//     circuits: BlockBasicCircuits<GoldilocksField, ZkSyncDefaultRoundFunction>,
//     object_store: &dyn ObjectStore,
//     aggregation_round: AggregationRound,
// ) -> Vec<(u8, String)> {
//     let circuits = circuits.into_flattened_set();
//     let mut ids_and_urls = Vec::with_capacity(circuits.len());
//     for (sequence_number, circuit) in circuits.into_iter().enumerate() {
//         let circuit_id = circuit.numeric_circuit_type();
//         let circuit_key = FriCircuitKey {
//             block_number,
//             sequence_number,
//             circuit_id,
//             aggregation_round,
//             depth: 0,
//         };
//         let blob_url = object_store
//             .put(circuit_key, &CircuitWrapper::Base(circuit))
//             .await
//             .unwrap();
//         ids_and_urls.push((circuit_id, blob_url));
//     }
//     ids_and_urls
// }
