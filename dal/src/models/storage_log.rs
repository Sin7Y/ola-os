use ola_types::{H256, U256};

#[derive(Debug, Clone, Copy)]
pub struct StorageTreeEntry {
    pub key: U256,
    pub value: H256,
    pub leaf_index: u64,
}