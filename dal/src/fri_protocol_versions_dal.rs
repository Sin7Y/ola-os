use ola_types::protocol_version::FriProtocolVersionId;

use crate::StorageProcessor;

#[derive(Debug)]
pub struct FriProtocolVersionsDal<'a, 'c> {
    pub storage: &'a mut StorageProcessor<'c>,
}

impl FriProtocolVersionsDal<'_, '_> {
    pub async fn save_prover_protocol_version(&mut self, id: FriProtocolVersionId) {
        sqlx::query!(
            r#"
            INSERT INTO
                prover_fri_protocol_versions (
                    id,
                    created_at
                )
            VALUES
                ($1, NOW())
            ON CONFLICT (id) DO NOTHING
            "#,
            id as i32
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }

    pub async fn protocol_versions(&mut self) -> Vec<FriProtocolVersionId> {
        sqlx::query!(
            r#"
            SELECT
                id
            FROM
                prover_fri_protocol_versions
            "#,
        )
        .fetch_all(self.storage.conn())
        .await
        .unwrap()
        .into_iter()
        .map(|row| FriProtocolVersionId::try_from(row.id as u16).unwrap())
        .collect()
    }
}
