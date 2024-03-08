use std::str::FromStr;

use ola_types::L1BatchNumber;
use strum::{Display, EnumString};
use tracing::Instrument;

use crate::{SqlxError, StorageProcessor};

#[derive(Debug, EnumString, Display)]
enum ProofVerificationStatus {
    #[strum(serialize = "not_ready")]
    NotReady,
    #[strum(serialize = "ready_to_be_verified")]
    ReadyToBeVerified,
    #[strum(serialize = "picked_by_offchain_verifier")]
    PickedByOffChainVerifier,
    #[strum(serialize = "offchain_verify_passed")]
    OffChainVerifyPassed,
    #[strum(serialize = "offchain_verify_failed")]
    OffChainVerifyFailed,
}

#[derive(Debug)]
pub struct ProofVerificationDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl ProofVerificationDal<'_, '_> {
    pub async fn insert_l1_batch_to_be_verified(
        &mut self,
        block_number: L1BatchNumber,
    ) -> Result<(), SqlxError> {
        sqlx::query!(
            r#"
            INSERT INTO
                proof_offchain_verification_details (l1_batch_number, status, created_at, updated_at)
            VALUES
                ($1, $2, NOW(), NOW())
            ON CONFLICT (l1_batch_number) DO NOTHING
            "#,
            block_number.0 as i64,
            ProofVerificationStatus::ReadyToBeVerified.to_string(),
        )
        .execute(self.storage.conn())
        .await?
        .rows_affected()
        .eq(&1)
        .then_some(())
        .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn mark_l1_batch_as_verified(
        &mut self,
        block_number: L1BatchNumber,
        is_passed: bool,
    ) -> Result<(), SqlxError> {
        let status = if is_passed {
            ProofVerificationStatus::OffChainVerifyPassed.to_string()
        } else {
            ProofVerificationStatus::OffChainVerifyFailed.to_string()
        };
        sqlx::query!(
            r#"
            UPDATE proof_offchain_verification_details
            SET
                status = $1,
                verifier_submit_at = NOW(),
                updated_at = NOW()
            WHERE
                l1_batch_number = $2
            "#,
            status,
            block_number.0 as i64,
        )
        .execute(self.storage.conn())
        .await?
        .rows_affected()
        .eq(&1)
        .then_some(())
        .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn mark_l1_batch_as_picked(
        &mut self,
        block_number: L1BatchNumber,
    ) -> Result<(), SqlxError> {
        sqlx::query!(
            r#"
            UPDATE proof_offchain_verification_details
            SET
                status = $1,
                verifier_picked_at = NOW(),
                updated_at = NOW()
            WHERE
                l1_batch_number = $2
            "#,
            ProofVerificationStatus::PickedByOffChainVerifier.to_string(),
            block_number.0 as i64,
        )
        .execute(self.storage.conn())
        .await?
        .rows_affected()
        .eq(&1)
        .then_some(())
        .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn get_last_l1_batch_verified(&mut self) -> sqlx::Result<L1BatchNumber> {
        let row = sqlx::query!(
            r#"
            SELECT
            COALESCE(MAX(l1_batch_number), 0) AS "number!"
            FROM
                proof_offchain_verification_details
            WHERE
                status IN ($1, $2)
            "#,
            ProofVerificationStatus::OffChainVerifyPassed.to_string(),
            ProofVerificationStatus::OffChainVerifyFailed.to_string(),
        )
        .fetch_one(self.storage.conn())
        .await?;

        Ok(L1BatchNumber(row.number as u32))
    }
    
    pub async fn get_l1_batch_verification_status(&mut self, l1_batch_number: L1BatchNumber) -> sqlx::Result<ProofVerificationStatus> {
        let row = sqlx::query!(
            r#"
            SELECT
                status
            FROM
                proof_offchain_verification_details
            WHERE
                l1_batch_number = $1
            "#,
            l1_batch_number.0 as i64,
        )
        .fetch_optional(self.storage.conn())
        .await?;

        Ok(row.map_or(ProofVerificationStatus::NotReady, |row| ProofVerificationStatus::from_str(&row.status).unwrap()))
    }
}
