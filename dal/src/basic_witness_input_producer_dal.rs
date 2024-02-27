use std::time::{Duration, Instant};

use sqlx::postgres::types::PgInterval;
use ola_types::L1BatchNumber;

use crate::{
    time_utils::{duration_to_naive_time, pg_interval_from_duration},
    StorageProcessor,
};

#[derive(Debug)]
pub struct BasicWitnessInputProducerDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

/// The amount of attempts to process a job before giving up.
pub const JOB_MAX_ATTEMPT: i16 = 10;

/// Time to wait for job to be processed
const JOB_PROCESSING_TIMEOUT: PgInterval = pg_interval_from_duration(Duration::from_secs(10 * 60));

/// Status of a job that the producer will work on.

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "basic_witness_input_producer_job_status")]
pub enum BasicWitnessInputProducerJobStatus {
    /// When the job is queued. Metadata calculator creates the job and marks it as queued.
    Queued,
    /// The job is not going to be processed. This state is designed for manual operations on DB.
    /// It is expected to be used if some jobs should be skipped like:
    /// - testing purposes (want to check a specific L1 Batch, I can mark everything before it skipped)
    /// - trim down costs on some environments (if I've done breaking changes,
    /// makes no sense to wait for everything to be processed, I can just skip them and save resources)
    ManuallySkipped,
    /// Currently being processed by one of the jobs. Transitory state, will transition to either
    /// [`BasicWitnessInputProducerStatus::Successful`] or [`BasicWitnessInputProducerStatus::Failed`].
    InProgress,
    /// The final (happy case) state we expect all jobs to end up. After the run is complete,
    /// the job uploaded it's inputs, it lands in successful.
    Successful,
    /// The job failed for reasons. It will be marked as such and the error persisted in DB.
    /// If it failed less than MAX_ATTEMPTs, the job will be retried,
    /// otherwise it will stay in this state as final state.
    Failed,
}

impl BasicWitnessInputProducerDal<'_, '_> {
    pub async fn create_basic_witness_input_producer_job(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO
                basic_witness_input_producer_jobs (l1_batch_number, status, created_at, updated_at)
            VALUES
                ($1, $2, NOW(), NOW())
            ON CONFLICT (l1_batch_number) DO NOTHING
            "#,
            l1_batch_number.0 as i64,
            BasicWitnessInputProducerJobStatus::Queued as BasicWitnessInputProducerJobStatus,
        )
        .execute(self.storage.conn())
        .await?;

        Ok(())
    }
}

/// These functions should only be used for tests.
impl BasicWitnessInputProducerDal<'_, '_> {
    pub async fn delete_all_jobs(&mut self) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM basic_witness_input_producer_jobs
            "#
        )
        .execute(self.storage.conn())
        .await?;
        Ok(())
    }
}
