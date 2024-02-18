use std::fmt;

pub use self::{
    block::{AccountTreeId, L1BatchNumber, L1ChainId, MiniblockNumber, PriorityOpId},
    l2::Nonce,
    protocol_version::U256,
};
use l2::L2TxCommonData;
pub use ola_basic_types::*;
use ola_utils::{bytes_to_u64s, h256_to_u64_array, hash::PoseidonBytes, u64s_to_bytes};
use protocol_version::ProtocolUpgradeTxCommonData;
use serde::{Deserialize, Serialize};

pub use storage::*;
use tx::execute::Execute;

pub mod api;
pub mod block;
pub mod circuit;
pub mod commitment;
pub mod events;
pub mod fee;
pub mod l2;
pub mod priority_op_onchain_data;
pub mod protocol_version;
pub mod prover_server_api;
pub mod request;
pub mod storage;
pub mod storage_writes_deduplicator;
pub mod system_contracts;
pub mod tokens;
pub mod tx;
pub mod utils;
pub mod vm_trace;

pub const EIP_712_TX_TYPE: u8 = 0x71;
pub const EIP_1559_TX_TYPE: u8 = 0x02;
pub const OLA_RAW_TX_TYPE: u8 = 0x10;

/// Denotes the first byte of the priority transaction.
pub const PRIORITY_OPERATION_L2_TX_TYPE: u8 = 0xff;

/// Denotes the first byte of the protocol upgrade transaction.
pub const PROTOCOL_UPGRADE_TX_TYPE: u8 = 0xfe;

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
    pub fn nonce(&self) -> Option<Nonce> {
        match &self.common_data {
            ExecuteTransactionCommon::L2(tx) => Some(tx.nonce),
            ExecuteTransactionCommon::ProtocolUpgrade(_) => None,
        }
    }

    pub fn hash(&self) -> H256 {
        match &self.common_data {
            ExecuteTransactionCommon::L2(data) => data.hash(),
            ExecuteTransactionCommon::ProtocolUpgrade(data) => data.hash(),
        }
    }

    pub fn initiator_account(&self) -> Address {
        match &self.common_data {
            ExecuteTransactionCommon::L2(data) => data.initiator_address,
            ExecuteTransactionCommon::ProtocolUpgrade(data) => data.sender,
        }
    }

    pub fn msg_hash(&self) -> Option<Vec<u8>> {
        let common_data = match &self.common_data {
            ExecuteTransactionCommon::L2(data) => data,
            ExecuteTransactionCommon::ProtocolUpgrade(_) => return None,
        };
        let chain_id = match common_data.extract_chain_id() {
            Some(chain) => chain as u64,
            None => return None,
        };

        let transaction_type = common_data.transaction_type as u64;
        let nonce = match self.nonce() {
            Some(n) => n.0 as u64,
            None => return None,
        };

        let from = h256_to_u64_array(&self.initiator_account()).to_vec();
        let to = h256_to_u64_array(&self.execute.contract_address).to_vec();
        let input = bytes_to_u64s(self.execute.calldata.clone());
        let data = vec![vec![chain_id, transaction_type, nonce], from, to, input]
            .iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        let msg = u64s_to_bytes(&data);
        let msg_hash = msg.hash_bytes();
        Some(msg_hash.to_vec())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecuteTransactionCommon {
    L2(L2TxCommonData),
    ProtocolUpgrade(ProtocolUpgradeTxCommonData),
}

impl fmt::Display for ExecuteTransactionCommon {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExecuteTransactionCommon::L2(data) => write!(f, "L2TxCommonData: {:?}", data),
            ExecuteTransactionCommon::ProtocolUpgrade(data) => {
                write!(f, "ProtocolUpgradeTxCommonData: {:?}", data)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputData {
    pub hash: H256,
    pub data: Vec<u8>,
}
