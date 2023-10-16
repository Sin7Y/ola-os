use l2::L2TxCommonData;
use serde::{Deserialize, Serialize};

pub use ola_basic_types::*;
pub use storage::*;
use tx::execute::Execute;

pub mod api;
pub mod fee;
pub mod l2;
pub mod request;
pub mod storage;
pub mod system_contracts;
pub mod tx;
pub mod utils;

#[derive(Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub common_data: ExecuteTransactionCommon,
    pub execute: Execute,
    pub received_timestamp_ms: u64,
}

impl std::fmt::Debug for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Transaction").field(&self.hash()).finish()
    }
}

impl PartialEq for Transaction {
    fn eq(&self, other: &Transaction) -> bool {
        self.hash() == other.hash()
    }
}

impl Eq for Transaction {}

impl Transaction {
    pub fn hash(&self) -> H256 {
        match &self.common_data {
            ExecuteTransactionCommon::L2(data) => data.hash(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecuteTransactionCommon {
    L2(L2TxCommonData),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputData {
    pub hash: H256,
    pub data: Vec<u8>,
}
