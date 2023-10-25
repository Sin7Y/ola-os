use ola_basic_types::{AccountTreeId, Address, L1BatchNumber, MiniblockNumber, H256, U256};
use ola_contracts::BaseSystemContractsHashes;
use serde::{Deserialize, Serialize};

use crate::{
    priority_op_onchain_data::PriorityOpOnchainData, protocol_version::ProtocolVersionId,
    Transaction,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeployedContract {
    pub account_id: AccountTreeId,
    pub bytecode: Vec<u8>,
}

impl DeployedContract {
    pub fn new(account_id: AccountTreeId, bytecode: Vec<u8>) -> Self {
        Self {
            account_id,
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
    /// Address of the fee account that was used when block was created
    pub fee_account_address: Address,
    /// Total number of processed priority operations in the block
    pub l1_tx_count: u16,
    /// Total number of processed txs that was requested offchain
    pub l2_tx_count: u16,
    /// The data of the processed priority operations hash which must be sent to the smart contract.
    pub priority_ops_onchain_data: Vec<PriorityOpOnchainData>,
    /// Preimages of the hashes that were sent as value of L2 logs by special system L2 contract.
    pub l2_to_l1_messages: Vec<Vec<u8>>,
    /// Hashes of contracts used this block
    pub used_contract_hashes: Vec<U256>,
    pub base_system_contracts_hashes: BaseSystemContractsHashes,
    /// Version of protocol used for the L1 batch.
    pub protocol_version: Option<ProtocolVersionId>,
}
