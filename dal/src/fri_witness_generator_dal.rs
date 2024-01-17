use ola_types::{protocol_version::FriProtocolVersionId, L1BatchNumber};

use crate::StorageProcessor;

#[derive(Debug)]
pub struct FriWitnessGeneratorDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
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
}
