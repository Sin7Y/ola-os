use std::{str::FromStr, time::Duration};

use ola_types::{
    basic_fri_types::CircuitIdRoundTuple,
    proofs::{AggregationRound, FriProverJobMetadata},
    protocol_version::FriProtocolVersionId,
    L1BatchNumber,
};
use strum::{Display, EnumString};

use crate::{time_utils::duration_to_naive_time, StorageProcessor};

#[derive(Debug, EnumString, Display)]
pub enum FriProofJobStatus {
    #[strum(serialize = "queued")]
    Queued,
    #[strum(serialize = "in_progress")]
    InProgress,
    #[strum(serialize = "successful")]
    Successful,
    #[strum(serialize = "failed")]
    Failed,
    #[strum(serialize = "sent_to_server")]
    SentToServer,
    #[strum(serialize = "skipped")]
    Skipped,
}

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
                        ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())
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
            FriProofJobStatus::Queued.to_string(),
        )
            .execute(self.storage.conn())
            .await
            .unwrap();
    }

    pub async fn save_proof(
        &mut self,
        id: u32,
        time_taken: Duration,
        blob_url: &str,
    ) -> FriProverJobMetadata {
        sqlx::query!(
            r#"
            UPDATE prover_jobs_fri
            SET
                status = $1,
                updated_at = NOW(),
                time_taken = $2,
                proof_blob_url = $3
            WHERE
                id = $4
            RETURNING
                prover_jobs_fri.id,
                prover_jobs_fri.l1_batch_number,
                prover_jobs_fri.circuit_id,
                prover_jobs_fri.aggregation_round,
                prover_jobs_fri.sequence_number,
                prover_jobs_fri.depth,
                prover_jobs_fri.is_node_final_proof
            "#,
            FriProofJobStatus::Successful.to_string(),
            duration_to_naive_time(time_taken),
            blob_url,
            id as i64,
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(|row| FriProverJobMetadata {
            id: row.id as u32,
            block_number: L1BatchNumber(row.l1_batch_number as u32),
            circuit_id: row.circuit_id as u8,
            aggregation_round: AggregationRound::try_from(row.aggregation_round as i32).unwrap(),
            sequence_number: row.sequence_number as usize,
            depth: row.depth as u16,
            is_node_final_proof: row.is_node_final_proof,
        })
        .unwrap()
    }

    pub async fn mark_proof_sent_to_server(&mut self, block_number: L1BatchNumber) {
        sqlx::query!(
            r#"
            UPDATE prover_jobs_fri
            SET
                status = $1,
                updated_at = NOW()
            WHERE
                l1_batch_number = $2
            "#,
            FriProofJobStatus::SentToServer.to_string(),
            block_number.0 as i64
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }

    pub async fn save_proof_error(&mut self, id: u32, error: String) {
        {
            sqlx::query!(
                r#"
                UPDATE prover_jobs_fri
                SET
                    status = $1,
                    error = $2,
                    updated_at = NOW()
                WHERE
                    id = $3
                "#,
                FriProofJobStatus::Failed.to_string(),
                error,
                id as i64,
            )
            .execute(self.storage.conn())
            .await
            .unwrap();
        }
    }

    pub async fn get_next_job(
        &mut self,
        protocol_versions: &[FriProtocolVersionId],
        picked_by: &str,
    ) -> Option<FriProverJobMetadata> {
        let protocol_versions: Vec<i32> = protocol_versions.iter().map(|&id| id as i32).collect();
        sqlx::query!(
            r#"
            UPDATE prover_jobs_fri
            SET
                status = $3,
                attempts = attempts + 1,
                updated_at = NOW(),
                processing_started_at = NOW(),
                picked_by = $2
            WHERE
                id = (
                    SELECT
                        id
                    FROM
                        prover_jobs_fri
                    WHERE
                        status = $4
                        AND protocol_version = ANY ($1)
                    ORDER BY
                        aggregation_round DESC,
                        l1_batch_number ASC,
                        id ASC
                    LIMIT
                        1
                    FOR UPDATE
                        SKIP LOCKED
                )
            RETURNING
                prover_jobs_fri.id,
                prover_jobs_fri.l1_batch_number,
                prover_jobs_fri.circuit_id,
                prover_jobs_fri.aggregation_round,
                prover_jobs_fri.sequence_number,
                prover_jobs_fri.depth,
                prover_jobs_fri.is_node_final_proof
            "#,
            &protocol_versions[..],
            picked_by,
            FriProofJobStatus::InProgress.to_string(),
            FriProofJobStatus::Queued.to_string(),
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(|row| FriProverJobMetadata {
            id: row.id as u32,
            block_number: L1BatchNumber(row.l1_batch_number as u32),
            circuit_id: row.circuit_id as u8,
            aggregation_round: AggregationRound::try_from(row.aggregation_round as i32).unwrap(),
            sequence_number: row.sequence_number as usize,
            depth: row.depth as u16,
            is_node_final_proof: row.is_node_final_proof,
        })
    }

    pub async fn get_prover_job_attempts(&mut self, id: u32) -> sqlx::Result<Option<u32>> {
        let attempts = sqlx::query!(
            r#"
            SELECT
                attempts
            FROM
                prover_jobs_fri
            WHERE
                id = $1
            "#,
            id as i64,
        )
        .fetch_optional(self.storage.conn())
        .await?
        .map(|row| row.attempts as u32);

        Ok(attempts)
    }

    pub async fn get_next_job_for_circuit_id_round(
        &mut self,
        circuits_to_pick: &[CircuitIdRoundTuple],
        protocol_versions: &[FriProtocolVersionId],
        picked_by: &str,
    ) -> Option<FriProverJobMetadata> {
        let circuit_ids: Vec<_> = circuits_to_pick
            .iter()
            .map(|tuple| tuple.circuit_id as i16)
            .collect();
        let protocol_versions: Vec<i32> = protocol_versions.iter().map(|&id| id as i32).collect();
        let aggregation_rounds: Vec<_> = circuits_to_pick
            .iter()
            .map(|tuple| tuple.aggregation_round as i16)
            .collect();
        sqlx::query!(
            r#"
            UPDATE prover_jobs_fri
            SET
                status = $5,
                attempts = attempts + 1,
                processing_started_at = NOW(),
                updated_at = NOW(),
                picked_by = $4
            WHERE
                id = (
                    SELECT
                        pj.id
                    FROM
                        (
                            SELECT
                                *
                            FROM
                                UNNEST($1::SMALLINT[], $2::SMALLINT[])
                        ) AS tuple (circuit_id, ROUND)
                        JOIN LATERAL (
                            SELECT
                                *
                            FROM
                                prover_jobs_fri AS pj
                            WHERE
                                pj.status = $6
                                AND pj.protocol_version = ANY ($3)
                                AND pj.circuit_id = tuple.circuit_id
                                AND pj.aggregation_round = tuple.round
                            ORDER BY
                                pj.l1_batch_number ASC,
                                pj.id ASC
                            LIMIT
                                1
                        ) AS pj ON TRUE
                    ORDER BY
                        pj.l1_batch_number ASC,
                        pj.aggregation_round DESC,
                        pj.id ASC
                    LIMIT
                        1
                    FOR UPDATE
                        SKIP LOCKED
                )
            RETURNING
                prover_jobs_fri.id,
                prover_jobs_fri.l1_batch_number,
                prover_jobs_fri.circuit_id,
                prover_jobs_fri.aggregation_round,
                prover_jobs_fri.sequence_number,
                prover_jobs_fri.depth,
                prover_jobs_fri.is_node_final_proof
            "#,
            &circuit_ids[..],
            &aggregation_rounds[..],
            &protocol_versions[..],
            picked_by,
            FriProofJobStatus::InProgress.to_string(),
            FriProofJobStatus::Queued.to_string(),
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(|row| FriProverJobMetadata {
            id: row.id as u32,
            block_number: L1BatchNumber(row.l1_batch_number as u32),
            circuit_id: row.circuit_id as u8,
            aggregation_round: AggregationRound::try_from(row.aggregation_round as i32).unwrap(),
            sequence_number: row.sequence_number as usize,
            depth: row.depth as u16,
            is_node_final_proof: row.is_node_final_proof,
        })
    }

    pub async fn get_least_proven_block_number_not_sent_to_server(
        &mut self,
    ) -> Option<(L1BatchNumber, FriProofJobStatus)> {
        let row = sqlx::query!(
            r#"
            SELECT
                l1_batch_number,
                status
            FROM
                prover_jobs_fri
            WHERE
                l1_batch_number = (
                    SELECT
                        MIN(l1_batch_number)
                    FROM
                        prover_jobs_fri
                    WHERE
                        status = $1
                        OR status = $2
                )
            "#,
            FriProofJobStatus::Successful.to_string(),
            FriProofJobStatus::Skipped.to_string()
        )
        .fetch_optional(self.storage.conn())
        .await
        .ok()?;
        match row {
            Some(row) => Some((
                L1BatchNumber(row.l1_batch_number as u32),
                FriProofJobStatus::from_str(&row.status).unwrap(),
            )),
            None => None,
        }
    }
}
