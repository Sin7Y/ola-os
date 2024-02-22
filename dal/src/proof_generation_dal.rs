use std::time::Duration;

use ola_types::L1BatchNumber;
use strum::{Display, EnumString};

use crate::{time_utils::pg_interval_from_duration, StorageProcessor};

#[derive(Debug)]
pub struct ProofGenerationDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

#[derive(Debug, EnumString, Display)]
enum ProofGenerationJobStatus {
    #[strum(serialize = "ready_to_be_proven")]
    ReadyToBeProven,
    #[strum(serialize = "picked_by_prover")]
    PickedByProver,
    #[strum(serialize = "generated")]
    Generated,
    #[strum(serialize = "skipped")]
    Skipped,
}

impl ProofGenerationDal<'_, '_> {
    pub async fn get_next_block_to_be_proven(
        &mut self,
        processing_timeout: Duration,
    ) -> Option<L1BatchNumber> {
        let processing_timeout = pg_interval_from_duration(processing_timeout);
        let result: Option<L1BatchNumber> = sqlx::query!(
            r#"
            UPDATE proof_generation_details
            SET
                status = 'picked_by_prover',
                updated_at = NOW(),
                prover_taken_at = NOW()
            WHERE
                l1_batch_number = (
                    SELECT
                        l1_batch_number
                    FROM
                        proof_generation_details
                    WHERE
                        status = 'ready_to_be_proven'
                        OR (
                            status = 'picked_by_prover'
                            AND prover_taken_at < NOW() - $1::INTERVAL
                        )
                    ORDER BY
                        l1_batch_number ASC
                    LIMIT
                        1
                    FOR UPDATE
                        SKIP LOCKED
                )
            RETURNING
                proof_generation_details.l1_batch_number
            "#,
            &processing_timeout,
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(|row| L1BatchNumber(row.l1_batch_number as u32));

        result
    }
}
