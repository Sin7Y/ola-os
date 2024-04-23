use std::collections::HashMap;
use std::fmt;

use ola_types::{api, events::VmEvent, tx::IncludedTxLocation, MiniblockNumber, H256};
use sqlx::types::chrono::Utc;

use crate::models::storage_event::StorageWeb3Log;
use crate::{SqlxError, StorageProcessor};
#[derive(Debug)]
struct EventTopic<'a>(Option<&'a H256>);

impl fmt::LowerHex for EventTopic<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(topic) = self.0 {
            fmt::LowerHex::fmt(topic, formatter)
        } else {
            Ok(()) // Don't write anything
        }
    }
}

#[derive(Debug)]
pub struct EventsDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl EventsDal<'_, '_> {
    /// Saves events for the specified miniblock.
    pub async fn save_events(
        &mut self,
        block_number: MiniblockNumber,
        all_block_events: &[(IncludedTxLocation, Vec<&VmEvent>)],
    ) {
        let mut copy = self
            .storage
            .conn()
            .copy_in_raw(
                "COPY events(
                    miniblock_number, tx_hash, tx_index_in_block, address,
                    event_index_in_block, event_index_in_tx,
                    topic1, topic2, topic3, topic4, value,
                    tx_initiator_address,
                    created_at, updated_at
                )
                FROM STDIN WITH (DELIMITER '|')",
            )
            .await
            .unwrap();

        let mut buffer = String::new();
        let now = Utc::now().naive_utc().to_string();
        let mut event_index_in_block = 0_u32;
        for (tx_location, events) in all_block_events {
            let IncludedTxLocation {
                tx_hash,
                tx_index_in_miniblock,
                tx_initiator_address,
            } = tx_location;

            for (event_index_in_tx, event) in events.iter().enumerate() {
                write_str!(
                    &mut buffer,
                    r"{block_number}|\\x{tx_hash:x}|{tx_index_in_miniblock}|\\x{address:x}|",
                    address = event.address
                );
                write_str!(&mut buffer, "{event_index_in_block}|{event_index_in_tx}|");
                write_str!(
                    &mut buffer,
                    r"\\x{topic0:x}|\\x{topic1:x}|\\x{topic2:x}|\\x{topic3:x}|",
                    topic0 = EventTopic(event.indexed_topics.get(0)),
                    topic1 = EventTopic(event.indexed_topics.get(1)),
                    topic2 = EventTopic(event.indexed_topics.get(2)),
                    topic3 = EventTopic(event.indexed_topics.get(3))
                );
                writeln_str!(
                    &mut buffer,
                    r"\\x{value}|\\x{tx_initiator_address:x}|{now}|{now}",
                    value = hex::encode(&event.value)
                );

                event_index_in_block += 1;
            }
        }
        copy.send(buffer.as_bytes()).await.unwrap();
        // note: all the time spent in this function is spent in `copy.finish()`
        copy.finish().await.unwrap();
    }

    /// Removes events with a block number strictly greater than the specified `block_number`.
    pub async fn rollback_events(&mut self, block_number: MiniblockNumber) {
        sqlx::query!(
            "DELETE FROM events WHERE miniblock_number > $1",
            block_number.0 as i64
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }
    pub(crate) async fn get_logs_by_tx_hashes(
        &mut self,
        hashes: &[H256],
    ) -> Result<HashMap<H256, Vec<api::Log>>, SqlxError> {
        let hashes = hashes
            .iter()
            .map(|hash| hash.as_bytes().to_vec())
            .collect::<Vec<_>>();
        let logs: Vec<_> = sqlx::query_as!(
            StorageWeb3Log,
            r#"
            SELECT
                address,
                topic1,
                topic2,
                topic3,
                topic4,
                value,
                NULL::bytea AS "block_hash",
                NULL::BIGINT AS "l1_batch_number?",
                miniblock_number,
                tx_hash,
                tx_index_in_block,
                event_index_in_block,
                event_index_in_tx
            FROM
                events
            WHERE
                tx_hash = ANY ($1)
            ORDER BY
                miniblock_number ASC,
                event_index_in_block ASC
            "#,
            &hashes[..],
        )
        .fetch_all(self.storage.conn())
        .await?;

        let mut result = HashMap::<H256, Vec<api::Log>>::new();

        for storage_log in logs {
            let current_log = api::Log::from(storage_log);
            let tx_hash = current_log.transaction_hash.unwrap();
            result.entry(tx_hash).or_default().push(current_log);
        }

        Ok(result)
    }

    pub(crate) async fn get_l2_to_l1_logs_by_hashes(
        &mut self,
        hashes: &[H256],
    ) -> Result<HashMap<H256, Vec<api::L2ToL1Log>>, SqlxError> {
        let hashes = &hashes
            .iter()
            .map(|hash| hash.as_bytes().to_vec())
            .collect::<Vec<_>>();
        let logs: Vec<_> = sqlx::query_as!(
            StorageL2ToL1Log,
            r#"
            SELECT
                miniblock_number,
                log_index_in_miniblock,
                log_index_in_tx,
                tx_hash,
                NULL::bytea AS "block_hash",
                NULL::BIGINT AS "l1_batch_number?",
                shard_id,
                is_service,
                tx_index_in_miniblock,
                tx_index_in_l1_batch,
                sender,
                key,
                value
            FROM
                l2_to_l1_logs
            WHERE
                tx_hash = ANY ($1)
            ORDER BY
                tx_index_in_l1_batch ASC,
                log_index_in_tx ASC
            "#,
            &hashes[..]
        )
        .fetch_all(self.storage.conn())
        .await?;

        let mut result = HashMap::<H256, Vec<api::L2ToL1Log>>::new();

        for storage_log in logs {
            let current_log = api::L2ToL1Log::from(storage_log);
            result
                .entry(current_log.transaction_hash)
                .or_default()
                .push(current_log);
        }

        Ok(result)
    }
}
