use std::collections::HashSet;

use ola_types::{log::LogQuery, AccountTreeId, Address, L1BatchNumber, StorageKey, H256};
use ola_utils::u256_to_h256;

use crate::StorageProcessor;

#[derive(Debug)]
pub struct StorageLogsDedupDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl StorageLogsDedupDal<'_, '_> {
    pub async fn insert_protective_reads(
        &mut self,
        l1_batch_number: L1BatchNumber,
        read_logs: &[LogQuery],
    ) {
        let mut copy = self
            .storage
            .conn()
            .copy_in_raw(
                "COPY protective_reads (l1_batch_number, address, key, created_at, updated_at) \
                FROM STDIN WITH (DELIMITER '|')",
            )
            .await
            .unwrap();

        let mut bytes: Vec<u8> = Vec::new();
        let now = Utc::now().naive_utc().to_string();
        for log in read_logs.iter() {
            let address_str = format!("\\\\x{}", hex::encode(log.address.0));
            let key_str = format!("\\\\x{}", hex::encode(u256_to_h256(log.key).0));
            let row = format!(
                "{}|{}|{}|{}|{}\n",
                l1_batch_number, address_str, key_str, now, now
            );
            bytes.extend_from_slice(row.as_bytes());
        }
        copy.send(bytes).await.unwrap();
        copy.finish().await.unwrap();
    }

    pub async fn insert_initial_writes(
        &mut self,
        l1_batch_number: L1BatchNumber,
        written_storage_keys: &[StorageKey],
    ) {
        let hashed_keys: Vec<_> = written_storage_keys
            .iter()
            .map(|key| StorageKey::raw_hashed_key(key.address(), key.key()).to_vec())
            .collect();

        let last_index = self
            .max_set_enumeration_index()
            .await
            .map(|(last_index, _)| last_index)
            .unwrap_or(0);
        let indices: Vec<_> = ((last_index + 1)..=(last_index + hashed_keys.len() as u64))
            .map(|x| x as i64)
            .collect();

        sqlx::query!(
            "INSERT INTO initial_writes (hashed_key, index, l1_batch_number, created_at, updated_at) \
            SELECT u.hashed_key, u.index, $3, now(), now() \
            FROM UNNEST($1::bytea[], $2::bigint[]) AS u(hashed_key, index)",
            &hashed_keys,
            &indices,
            l1_batch_number.0 as i64,
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }

    pub async fn get_protective_reads_for_l1_batch(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) -> HashSet<StorageKey> {
        sqlx::query!(
            "SELECT address, key FROM protective_reads WHERE l1_batch_number = $1",
            l1_batch_number.0 as i64
        )
        .fetch_all(self.storage.conn())
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            StorageKey::new(
                AccountTreeId::new(Address::from_slice(&row.address)),
                H256::from_slice(&row.key),
            )
        })
        .collect()
    }

    pub async fn max_set_enumeration_index(&mut self) -> Option<(u64, L1BatchNumber)> {
        sqlx::query!(
            "SELECT index, l1_batch_number FROM initial_writes \
            WHERE index IS NOT NULL \
            ORDER BY index DESC LIMIT 1",
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(|row| {
            (
                row.index.unwrap() as u64,
                L1BatchNumber(row.l1_batch_number as u32),
            )
        })
    }
}
