use ola_types::{snapshots::SnapshotRecoveryStatus, L1BatchNumber, MiniblockNumber, H256};

use crate::StorageProcessor;

#[derive(Debug)]
pub struct SnapshotRecoveryDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl SnapshotRecoveryDal<'_, '_> {
    pub async fn set_applied_snapshot_status(
        &mut self,
        status: &SnapshotRecoveryStatus,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO
                snapshot_recovery (
                    l1_batch_number,
                    l1_batch_root_hash,
                    miniblock_number,
                    miniblock_root_hash,
                    last_finished_chunk_id,
                    total_chunk_count,
                    updated_at,
                    created_at
                )
            VALUES
                ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            ON CONFLICT (l1_batch_number) DO
            UPDATE
            SET
                l1_batch_number = excluded.l1_batch_number,
                l1_batch_root_hash = excluded.l1_batch_root_hash,
                miniblock_number = excluded.miniblock_number,
                miniblock_root_hash = excluded.miniblock_root_hash,
                last_finished_chunk_id = excluded.last_finished_chunk_id,
                total_chunk_count = excluded.total_chunk_count,
                updated_at = excluded.updated_at
            "#,
            status.l1_batch_number.0 as i64,
            status.l1_batch_root_hash.0.as_slice(),
            status.miniblock_number.0 as i64,
            status.miniblock_root_hash.0.as_slice(),
            status.last_finished_chunk_id.map(|v| v as i32),
            status.total_chunk_count as i64,
        )
        .execute(self.storage.conn())
        .await?;
        Ok(())
    }

    pub async fn get_applied_snapshot_status(
        &mut self,
    ) -> sqlx::Result<Option<SnapshotRecoveryStatus>> {
        let record = sqlx::query!(
            r#"
            SELECT
                l1_batch_number,
                l1_batch_root_hash,
                miniblock_number,
                miniblock_root_hash,
                last_finished_chunk_id,
                total_chunk_count
            FROM
                snapshot_recovery
            "#,
        )
        .fetch_optional(self.storage.conn())
        .await?;

        Ok(record.map(|r| SnapshotRecoveryStatus {
            l1_batch_number: L1BatchNumber(r.l1_batch_number as u32),
            l1_batch_root_hash: H256::from_slice(&r.l1_batch_root_hash),
            miniblock_number: MiniblockNumber(r.miniblock_number as u32),
            miniblock_root_hash: H256::from_slice(&r.miniblock_root_hash),
            last_finished_chunk_id: r.last_finished_chunk_id.map(|v| v as u64),
            total_chunk_count: r.total_chunk_count as u64,
        }))
    }
}
