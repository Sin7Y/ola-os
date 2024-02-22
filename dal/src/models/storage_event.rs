use ola_types::{api::Log, Address, Bytes, Index, H256, U256, U64};

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct StorageWeb3Log {
    pub address: Vec<u8>,
    pub topic1: Vec<u8>,
    pub topic2: Vec<u8>,
    pub topic3: Vec<u8>,
    pub topic4: Vec<u8>,
    pub value: Vec<u8>,
    pub block_hash: Option<Vec<u8>>,
    pub miniblock_number: i64,
    pub l1_batch_number: Option<i64>,
    pub tx_hash: Vec<u8>,
    pub tx_index_in_block: i32,
    pub event_index_in_block: i32,
    pub event_index_in_tx: i32,
}

impl From<StorageWeb3Log> for Log {
    fn from(log: StorageWeb3Log) -> Log {
        let topics = vec![log.topic1, log.topic2, log.topic3, log.topic4]
            .into_iter()
            .filter_map(|topic| {
                if !topic.is_empty() {
                    Some(H256::from_slice(&topic))
                } else {
                    None
                }
            })
            .collect();
        Log {
            address: Address::from_slice(&log.address),
            topics,
            data: Bytes(log.value),
            block_hash: log.block_hash.map(|hash| H256::from_slice(&hash)),
            block_number: Some(U64::from(log.miniblock_number as u32)),
            l1_batch_number: log.l1_batch_number.map(U64::from),
            transaction_hash: Some(H256::from_slice(&log.tx_hash)),
            transaction_index: Some(Index::from(log.tx_index_in_block as u32)),
            log_index: Some(U256::from(log.event_index_in_block as u32)),
            transaction_log_index: Some(U256::from(log.event_index_in_block as u32)),
            log_type: None,
            removed: Some(false),
        }
    }
}
