use std::collections::HashMap;

use ola_types::{
    log::StorageLog, AccountTreeId, Address, L1BatchNumber, MiniblockNumber, StorageKey, H256, U256,
};
use sqlx::types::chrono::Utc;

use crate::{models::storage_log::StorageTreeEntry, StorageProcessor};

#[derive(Debug)]
pub struct StorageLogsDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl StorageLogsDal<'_, '_> {
    /// Inserts storage logs grouped by transaction for a miniblock. The ordering of transactions
    /// must be the same as their ordering in the miniblock.
    pub async fn insert_storage_logs(
        &mut self,
        block_number: MiniblockNumber,
        logs: &[(H256, Vec<StorageLog>)],
    ) {
        self.insert_storage_logs_inner(block_number, logs, 0).await;
    }

    async fn insert_storage_logs_inner(
        &mut self,
        block_number: MiniblockNumber,
        logs: &[(H256, Vec<StorageLog>)],
        mut operation_number: u32,
    ) {
        let mut copy = self
            .storage
            .conn()
            .copy_in_raw(
                "COPY storage_logs(
                    hashed_key, address, key, value, operation_number, tx_hash, miniblock_number,
                    created_at, updated_at
                )
                FROM STDIN WITH (DELIMITER '|')",
            )
            .await
            .unwrap();

        let mut buffer = String::new();
        let now = Utc::now().naive_utc().to_string();
        for (tx_hash, logs) in logs {
            for log in logs {
                write_str!(
                    &mut buffer,
                    r"\\x{hashed_key:x}|\\x{address:x}|\\x{key:x}|\\x{value:x}|",
                    hashed_key = log.key.hashed_key(),
                    address = log.key.address(),
                    key = log.key.key(),
                    value = log.value
                );
                writeln_str!(
                    &mut buffer,
                    r"{operation_number}|\\x{tx_hash:x}|{block_number}|{now}|{now}"
                );

                operation_number += 1;
            }
        }
        copy.send(buffer.as_bytes()).await.unwrap();
        copy.finish().await.unwrap();
    }

    pub async fn get_touched_slots_for_l1_batch(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) -> HashMap<StorageKey, H256> {
        let rows = sqlx::query!(
            "SELECT address, key, value \
            FROM storage_logs \
            WHERE miniblock_number BETWEEN \
                (SELECT MIN(number) FROM miniblocks WHERE l1_batch_number = $1) \
                AND (SELECT MAX(number) FROM miniblocks WHERE l1_batch_number = $1) \
            ORDER BY miniblock_number, operation_number",
            l1_batch_number.0 as i64
        )
        .fetch_all(self.storage.conn())
        .await
        .unwrap();

        let touched_slots = rows.into_iter().map(|row| {
            let key = StorageKey::new(
                AccountTreeId::new(Address::from_slice(&row.address)),
                H256::from_slice(&row.key),
            );
            (key, H256::from_slice(&row.value))
        });
        touched_slots.collect()
    }

    pub async fn get_l1_batches_and_indices_for_initial_writes(
        &mut self,
        hashed_keys: &[H256],
    ) -> HashMap<H256, (L1BatchNumber, u64)> {
        if hashed_keys.is_empty() {
            return HashMap::new(); // Shortcut to save time on communication with DB in the common case
        }

        let hashed_keys: Vec<_> = hashed_keys.iter().map(H256::as_bytes).collect();
        let rows = sqlx::query!(
            r#"
            SELECT
                hashed_key,
                l1_batch_number,
                INDEX
            FROM
                initial_writes
            WHERE
                hashed_key = ANY ($1::bytea[])
            "#,
            &hashed_keys as &[&[u8]],
        )
        .fetch_all(self.storage.conn())
        .await
        .unwrap();

        rows.into_iter()
            .map(|row| {
                (
                    H256::from_slice(&row.hashed_key),
                    (
                        L1BatchNumber(row.l1_batch_number as u32),
                        row.index.map_or(0, |n| n as u64),
                    ),
                )
            })
            .collect()
    }

    pub async fn resolve_hashed_keys(&mut self, hashed_keys: &[H256]) -> Vec<StorageKey> {
        let hashed_keys: Vec<_> = hashed_keys.iter().map(H256::as_bytes).collect();
        sqlx::query!(
            "SELECT \
                (SELECT ARRAY[address,key] FROM storage_logs \
                WHERE hashed_key = u.hashed_key \
                ORDER BY miniblock_number, operation_number \
                LIMIT 1) as \"address_and_key?\" \
            FROM UNNEST($1::bytea[]) AS u(hashed_key)",
            &hashed_keys as &[&[u8]],
        )
        .fetch_all(self.storage.conn())
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            let address_and_key = row.address_and_key.unwrap();
            StorageKey::new(
                AccountTreeId::new(Address::from_slice(&address_and_key[0])),
                H256::from_slice(&address_and_key[1]),
            )
        })
        .collect()
    }

    async fn get_storage_values(
        &mut self,
        hashed_keys: &[H256],
        miniblock_number: MiniblockNumber,
    ) -> HashMap<H256, Option<H256>> {
        let hashed_keys: Vec<_> = hashed_keys.iter().map(H256::as_bytes).collect();

        let rows = sqlx::query!(
            "SELECT u.hashed_key as \"hashed_key!\", \
                (SELECT value FROM storage_logs \
                WHERE hashed_key = u.hashed_key AND miniblock_number <= $2 \
                ORDER BY miniblock_number DESC, operation_number DESC LIMIT 1) as \"value?\" \
            FROM UNNEST($1::bytea[]) AS u(hashed_key)",
            &hashed_keys as &[&[u8]],
            miniblock_number.0 as i64
        )
        .fetch_all(self.storage.conn())
        .await
        .unwrap();

        rows.into_iter()
            .map(|row| {
                let key = H256::from_slice(&row.hashed_key);
                let value = row.value.map(|value| H256::from_slice(&value));
                (key, value)
            })
            .collect()
    }

    pub async fn get_previous_storage_values(
        &mut self,
        hashed_keys: &[H256],
        next_l1_batch: L1BatchNumber,
    ) -> HashMap<H256, Option<H256>> {
        let (miniblock_number, _) = self
            .storage
            .blocks_dal()
            .get_miniblock_range_of_l1_batch(next_l1_batch)
            .await
            .unwrap();

        if miniblock_number == MiniblockNumber(0) {
            hashed_keys.iter().copied().map(|key| (key, None)).collect()
        } else {
            self.get_storage_values(hashed_keys, miniblock_number - 1)
                .await
        }
    }

    /// Counts the total number of storage logs in the specified miniblock,
    // TODO(PLA-596): add storage log count to snapshot metadata instead?
    pub async fn count_miniblock_storage_logs(
        &mut self,
        miniblock_number: MiniblockNumber,
    ) -> sqlx::Result<u64> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM storage_logs WHERE miniblock_number = $1",
            miniblock_number.0 as i32
        )
        .fetch_one(self.storage.conn())
        .await?;
        Ok(count.unwrap_or(0) as u64)
    }

    /// Gets a starting tree entry for each of the supplied `key_ranges` for the specified
    /// `miniblock_number`. This method is used during Merkle tree recovery.
    pub async fn get_chunk_starts_for_miniblock(
        &mut self,
        miniblock_number: MiniblockNumber,
        key_ranges: &[core::ops::RangeInclusive<H256>],
    ) -> sqlx::Result<Vec<Option<StorageTreeEntry>>> {
        let (start_keys, end_keys): (Vec<_>, Vec<_>) = key_ranges
            .iter()
            .map(|range| (range.start().as_bytes(), range.end().as_bytes()))
            .unzip();
        let rows = sqlx::query!(
            r#"
            WITH
                sl AS (
                    SELECT
                        (
                            SELECT
                                ARRAY[hashed_key, value] AS kv
                            FROM
                                storage_logs
                            WHERE
                                storage_logs.miniblock_number = $1
                                AND storage_logs.hashed_key >= u.start_key
                                AND storage_logs.hashed_key <= u.end_key
                            ORDER BY
                                storage_logs.hashed_key
                            LIMIT
                                1
                        )
                    FROM
                        UNNEST($2::bytea[], $3::bytea[]) AS u (start_key, end_key)
                )
            SELECT
                sl.kv[1] AS "hashed_key?",
                sl.kv[2] AS "value?",
                initial_writes.index
            FROM
                sl
                LEFT OUTER JOIN initial_writes ON initial_writes.hashed_key = sl.kv[1]
            "#,
            miniblock_number.0 as i64,
            &start_keys as &[&[u8]],
            &end_keys as &[&[u8]],
        )
        .fetch_all(self.storage.conn())
        .await?;

        let rows = rows.into_iter().map(|row| {
            Some(StorageTreeEntry {
                key: U256::from_little_endian(row.hashed_key.as_ref()?),
                value: H256::from_slice(row.value.as_ref()?),
                leaf_index: row.index? as u64,
            })
        });
        Ok(rows.collect())
    }

    /// Fetches tree entries for the specified `miniblock_number` and `key_range`. This is used during
    /// Merkle tree recovery.
    pub async fn get_tree_entries_for_miniblock(
        &mut self,
        miniblock_number: MiniblockNumber,
        key_range: core::ops::RangeInclusive<H256>,
    ) -> sqlx::Result<Vec<StorageTreeEntry>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                storage_logs.hashed_key,
                storage_logs.value,
                initial_writes.index
            FROM
                storage_logs
                INNER JOIN initial_writes ON storage_logs.hashed_key = initial_writes.hashed_key
            WHERE
                storage_logs.miniblock_number = $1
                AND storage_logs.hashed_key >= $2::bytea
                AND storage_logs.hashed_key <= $3::bytea
            ORDER BY
                storage_logs.hashed_key
            "#,
            miniblock_number.0 as i64,
            key_range.start().as_bytes(),
            key_range.end().as_bytes()
        )
        .fetch_all(self.storage.conn())
        .await?;

        let rows = rows.into_iter().map(|row| StorageTreeEntry {
            key: U256::from_little_endian(&row.hashed_key),
            value: H256::from_slice(&row.value),
            leaf_index: row.index.map_or(0, |n| n as u64),
        });
        Ok(rows.collect())
    }
}
