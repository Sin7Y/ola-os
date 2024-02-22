use std::time::Duration;

use ola_types::{protocol_version::FriProtocolVersionId, L1BatchNumber};

use crate::{time_utils::duration_to_naive_time, StorageProcessor};

#[derive(Debug)]
pub struct FriWitnessGeneratorDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

#[derive(Debug, strum::Display, strum::EnumString, strum::AsRefStr)]
pub enum FriWitnessJobStatus {
    #[strum(serialize = "failed")]
    Failed,
    #[strum(serialize = "skipped")]
    Skipped,
    #[strum(serialize = "successful")]
    Successful,
    #[strum(serialize = "in_progress")]
    InProgress,
    #[strum(serialize = "queued")]
    Queued,
}

impl FriWitnessGeneratorDal<'_, '_> {
    pub async fn save_witness_inputs(
        &mut self,
        block_number: L1BatchNumber,
        object_key: &str,
        protocol_version_id: FriProtocolVersionId,
    ) {
        sqlx::query!(
            r#"
            INSERT INTO
                witness_inputs_fri (
                    l1_batch_number,
                    merkle_tree_paths_blob_url,
                    protocol_version,
                    status,
                    created_at,
                    updated_at
                )
            VALUES
                ($1, $2, $3, 'queued', NOW(), NOW())
            ON CONFLICT (l1_batch_number) DO NOTHING
            "#,
            block_number.0 as i64,
            object_key,
            protocol_version_id as i32,
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap();
    }

    pub async fn get_next_basic_circuit_witness_job(
        &mut self,
        last_l1_batch_to_process: u32,
        protocol_versions: &[FriProtocolVersionId],
        picked_by: &str,
    ) -> Option<L1BatchNumber> {
        let protocol_versions: Vec<i32> = protocol_versions.iter().map(|&id| id as i32).collect();
        sqlx::query!(
            r#"
            UPDATE witness_inputs_fri
            SET
                status = 'in_progress',
                attempts = attempts + 1,
                updated_at = NOW(),
                processing_started_at = NOW(),
                picked_by = $3
            WHERE
                l1_batch_number = (
                    SELECT
                        l1_batch_number
                    FROM
                        witness_inputs_fri
                    WHERE
                        l1_batch_number <= $1
                        AND status = 'queued'
                        AND protocol_version = ANY ($2)
                    ORDER BY
                        l1_batch_number ASC
                    LIMIT
                        1
                    FOR UPDATE
                        SKIP LOCKED
                )
            RETURNING
                witness_inputs_fri.l1_batch_number
            "#,
            last_l1_batch_to_process as i64,
            &protocol_versions[..],
            picked_by,
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(|row| L1BatchNumber(row.l1_batch_number as u32))
    }

    pub async fn get_basic_circuit_witness_job_attempts(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) -> sqlx::Result<Option<u32>> {
        let attempts = sqlx::query!(
            r#"
            SELECT
                attempts
            FROM
                witness_inputs_fri
            WHERE
                l1_batch_number = $1
            "#,
            l1_batch_number.0 as i64,
        )
        .fetch_optional(self.storage.conn())
        .await?
        .map(|row| row.attempts as u32);

        Ok(attempts)
    }

    pub async fn mark_witness_job(
        &mut self,
        status: FriWitnessJobStatus,
        block_number: L1BatchNumber,
    ) {
        sqlx::query!(
            r#"
            UPDATE witness_inputs_fri
            SET
                status = $1,
                updated_at = NOW()
            WHERE
                l1_batch_number = $2
            "#,
            format!("{}", status),
            block_number.0 as i64
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }

    pub async fn mark_witness_job_as_successful(
        &mut self,
        block_number: L1BatchNumber,
        time_taken: Duration,
    ) {
        sqlx::query!(
            r#"
            UPDATE witness_inputs_fri
            SET
                status = 'successful',
                updated_at = NOW(),
                time_taken = $1
            WHERE
                l1_batch_number = $2
            "#,
            duration_to_naive_time(time_taken),
            block_number.0 as i64
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }

    pub async fn mark_witness_job_failed(&mut self, error: &str, block_number: L1BatchNumber) {
        sqlx::query!(
            r#"
            UPDATE witness_inputs_fri
            SET
                status = 'failed',
                error = $1,
                updated_at = NOW()
            WHERE
                l1_batch_number = $2
            "#,
            error,
            block_number.0 as i64
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }

    pub async fn protocol_version_for_l1_batch(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) -> FriProtocolVersionId {
        sqlx::query!(
            r#"
            SELECT
                protocol_version
            FROM
                witness_inputs_fri
            WHERE
                l1_batch_number = $1
            "#,
            l1_batch_number.0 as i64,
        )
        .fetch_one(self.storage.conn())
        .await
        .unwrap()
        .protocol_version
        .map(|id| FriProtocolVersionId::try_from(id as u16).unwrap())
        .unwrap()
    }
}
