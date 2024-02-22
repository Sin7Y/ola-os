use ola_types::{proofs::AggregationRound, protocol_version::FriProtocolVersionId, L1BatchNumber};

use crate::StorageProcessor;

#[derive(Debug)]
pub struct FriProverDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl FriProverDal<'_, '_> {
    pub async fn insert_prover_jobs(
        &mut self,
        l1_batch_number: L1BatchNumber,
        circuit_ids_and_urls: Vec<(u8, String)>,
        aggregation_round: AggregationRound,
        depth: u16,
        protocol_version_id: FriProtocolVersionId,
    ) {
        // let latency = MethodLatency::new("save_fri_prover_jobs");
        for (sequence_number, (circuit_id, circuit_blob_url)) in
            circuit_ids_and_urls.iter().enumerate()
        {
            self.insert_prover_job(
                l1_batch_number,
                *circuit_id,
                depth,
                sequence_number,
                aggregation_round,
                circuit_blob_url,
                false,
                protocol_version_id,
            )
            .await;
        }
        // drop(latency);
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_prover_job(
        &mut self,
        l1_batch_number: L1BatchNumber,
        circuit_id: u8,
        depth: u16,
        sequence_number: usize,
        aggregation_round: AggregationRound,
        circuit_blob_url: &str,
        is_node_final_proof: bool,
        protocol_version_id: FriProtocolVersionId,
    ) {
        sqlx::query!(
                    r#"
                    INSERT INTO
                        prover_jobs_fri (
                            l1_batch_number,
                            circuit_id,
                            circuit_blob_url,
                            aggregation_round,
                            sequence_number,
                            depth,
                            is_node_final_proof,
                            protocol_version,
                            status,
                            created_at,
                            updated_at
                        )
                    VALUES
                        ($1, $2, $3, $4, $5, $6, $7, $8, 'queued', NOW(), NOW())
                    ON CONFLICT (l1_batch_number, aggregation_round, circuit_id, depth, sequence_number) DO
                    UPDATE
                    SET
                        updated_at = NOW()
                    "#,
            l1_batch_number.0 as i64,
            circuit_id as i16,
            circuit_blob_url,
            aggregation_round as i64,
            sequence_number as i64,
            depth as i32,
            is_node_final_proof,
            protocol_version_id as i32,
        )
            .execute(self.storage.conn())
            .await
            .unwrap();
    }
}
