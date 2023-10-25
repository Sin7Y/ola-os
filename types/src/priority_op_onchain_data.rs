use ola_basic_types::{H256, U256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriorityOpOnchainData {
    pub onchain_data_hash: H256,
}

impl From<PriorityOpOnchainData> for Vec<u8> {
    fn from(data: PriorityOpOnchainData) -> Vec<u8> {
        let mut raw_data = vec![0u8; 32];
        raw_data.copy_from_slice(data.onchain_data_hash.as_bytes());
        raw_data
    }
}

impl From<Vec<u8>> for PriorityOpOnchainData {
    fn from(data: Vec<u8>) -> Self {
        Self {
            onchain_data_hash: H256::from_slice(&data),
        }
    }
}
