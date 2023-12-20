pub use ola_basic_types::{
    AccountTreeId, L1BatchNumber, L1ChainId, MiniblockNumber, PriorityOpId, H256, U256,
};
use ola_contracts::BaseSystemContractsHashes;
use serde::{Deserialize, Serialize};

use crate::{protocol_version::ProtocolVersionId, Transaction};

// use olavm_exe_core::merkle_tree::log::WitnessStorageLog;
use super::storage::log::WitnessStorageLog;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeployedContract {
    pub account_id: AccountTreeId,
    pub raw: Vec<u8>,
    pub bytecode: Vec<u8>,
}

impl DeployedContract {
    pub fn new(account_id: AccountTreeId, raw: Vec<u8>, bytecode: Vec<u8>) -> Self {
        Self {
            account_id,
            raw,
            bytecode,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MiniblockHeader {
    pub number: MiniblockNumber,
    pub timestamp: u64,
    pub hash: H256,
    pub l1_tx_count: u16,
    pub l2_tx_count: u16,
    pub base_system_contracts_hashes: BaseSystemContractsHashes,
    pub protocol_version: Option<ProtocolVersionId>,
}

#[derive(Debug)]
pub struct MiniblockReexecuteData {
    pub number: MiniblockNumber,
    pub timestamp: u64,
    pub txs: Vec<Transaction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct L1BatchHeader {
    /// Numeric ID of the block. Starts from 1, 0 block is considered genesis block and has no transactions.
    pub number: L1BatchNumber,
    /// Whether block is sealed or not (doesn't correspond to committing/verifying it on the L1).
    pub is_finished: bool,
    /// Timestamp when block was first created.
    pub timestamp: u64,
    /// Total number of processed priority operations in the block
    pub l1_tx_count: u16,
    /// Total number of processed txs that was requested offchain
    pub l2_tx_count: u16,
    /// Hashes of contracts used this block
    pub used_contract_hashes: Vec<U256>,
    pub base_system_contracts_hashes: BaseSystemContractsHashes,
    /// Version of protocol used for the L1 batch.
    pub protocol_version: Option<ProtocolVersionId>,
}

impl L1BatchHeader {
    pub fn new(
        number: L1BatchNumber,
        timestamp: u64,
        base_system_contracts_hashes: BaseSystemContractsHashes,
        protocol_version: ProtocolVersionId,
    ) -> L1BatchHeader {
        Self {
            number,
            is_finished: false,
            timestamp,
            l1_tx_count: 0,
            l2_tx_count: 0,
            used_contract_hashes: vec![],
            base_system_contracts_hashes,
            protocol_version: Some(protocol_version),
        }
    }
}

pub struct WitnessBlockWithLogs {
    pub header: L1BatchHeader,
    pub storage_logs: Vec<WitnessStorageLog>,
}
