use std::fmt;

use ola_types::{events::VmEvent, tx::IncludedTxLocation, MiniblockNumber, H256};
use sqlx::types::chrono::Utc;

use crate::StorageProcessor;

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
}
