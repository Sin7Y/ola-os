use ola_basic_types::{L1BatchNumber, MiniblockNumber, H256};

#[derive(Debug, PartialEq)]
pub struct SnapshotRecoveryStatus {
    pub l1_batch_number: L1BatchNumber,
    pub l1_batch_root_hash: H256,
    pub miniblock_number: MiniblockNumber,
    pub miniblock_root_hash: H256,
    pub last_finished_chunk_id: Option<u64>,
    pub total_chunk_count: u64,
}
