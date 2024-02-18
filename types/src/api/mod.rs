pub use crate::request::{SerializationTransactionError, TransactionRequest};
use chrono::{DateTime, Utc};
use ola_basic_types::{Address, Bytes, Index, H256, U256, U64};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use strum::Display;

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize, Display)]
#[serde(untagged)]
pub enum BlockId {
    /// By Hash
    Hash(H256),
    /// By Number
    Number(BlockNumber),
}

impl BlockId {
    /// Extract block's id variant name.
    pub fn extract_block_tag(&self) -> String {
        match self {
            BlockId::Number(block_number) => block_number.to_string(),
            BlockId::Hash(_) => "hash".to_string(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Display)]
pub enum BlockNumber {
    /// Alias for BlockNumber::Latest.
    Committed,
    /// Last block that was finalized on L1.
    // Finalized,
    /// Latest sealed block
    Latest,
    /// Earliest block (genesis)
    Earliest,
    /// Latest block (may be the block that is currently open).
    Pending,
    /// Block by number from canon chain
    Number(U64),
}

impl<T: Into<U64>> From<T> for BlockNumber {
    fn from(num: T) -> Self {
        BlockNumber::Number(num.into())
    }
}

impl Serialize for BlockNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            BlockNumber::Number(ref x) => serializer.serialize_str(&format!("0x{:x}", x)),
            BlockNumber::Committed => serializer.serialize_str("committed"),
            // BlockNumber::Finalized => serializer.serialize_str("finalized"),
            BlockNumber::Latest => serializer.serialize_str("latest"),
            BlockNumber::Earliest => serializer.serialize_str("earliest"),
            BlockNumber::Pending => serializer.serialize_str("pending"),
        }
    }
}

impl<'de> Deserialize<'de> for BlockNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = BlockNumber;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("A block number or one of the supported aliases")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                let result = match value {
                    "committed" => BlockNumber::Committed,
                    // "finalized" => BlockNumber::Finalized,
                    "latest" => BlockNumber::Latest,
                    "earliest" => BlockNumber::Earliest,
                    "pending" => BlockNumber::Pending,
                    num => {
                        let number =
                            U64::deserialize(de::value::BorrowedStrDeserializer::new(num))?;
                        BlockNumber::Number(number)
                    }
                };

                Ok(result)
            }
        }
        deserializer.deserialize_str(V)
    }
}

/// Helper struct for EIP-1898.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockNumberObject {
    pub block_number: BlockNumber,
}

/// Helper struct for EIP-1898.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockHashObject {
    pub block_hash: H256,
}

/// Helper enum for EIP-1898.
/// Should be used for `block` parameters in web3 JSON RPC methods that implement EIP-1898.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockIdVariant {
    BlockNumber(BlockNumber),
    BlockNumberObject(BlockNumberObject),
    BlockHashObject(BlockHashObject),
}

impl From<BlockIdVariant> for BlockId {
    fn from(value: BlockIdVariant) -> BlockId {
        match value {
            BlockIdVariant::BlockNumber(number) => BlockId::Number(number),
            BlockIdVariant::BlockNumberObject(number_object) => {
                BlockId::Number(number_object.block_number)
            }
            BlockIdVariant::BlockHashObject(hash_object) => BlockId::Hash(hash_object.block_hash),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransactionStatus {
    Pending,
    Included,
    Verified,
    Failed,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionReceipt {
    /// Transaction hash.
    #[serde(rename = "transactionHash")]
    pub transaction_hash: H256,
    /// Index within the block.
    #[serde(rename = "transactionIndex")]
    pub transaction_index: Index,
    /// Hash of the block this transaction was included within.
    #[serde(rename = "blockHash")]
    pub block_hash: Option<H256>,
    /// Number of the miniblock this transaction was included within.
    #[serde(rename = "blockNumber")]
    pub block_number: Option<U64>,
    /// Index of transaction in l1 batch
    #[serde(rename = "l1BatchTxIndex")]
    pub l1_batch_tx_index: Option<Index>,
    /// Number of the l1 batch this transaction was included within.
    #[serde(rename = "l1BatchNumber")]
    pub l1_batch_number: Option<U64>,
    /// Sender
    /// Note: default address if the client did not return this value
    #[serde(default)]
    pub from: Address,
    /// Recipient (None when contract creation)
    /// Note: Also `None` if the client did not return this value
    #[serde(default)]
    pub to: Option<Address>,
    /// Cumulative gas used within the block after this was executed.
    // #[serde(rename = "cumulativeGasUsed")]
    // pub cumulative_gas_used: U256,
    /// Gas used by this transaction alone.
    ///
    /// Gas used is `None` if the the client is running in light client mode.
    // #[serde(rename = "gasUsed")]
    // pub gas_used: Option<U256>,
    /// Contract address created, or `None` if not a deployment.
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<Address>,
    /// Logs generated within this transaction.
    pub logs: Vec<Log>,
    /// L2 to L1 logs generated within this transaction.
    // #[serde(rename = "l2ToL1Logs")]
    // pub l2_to_l1_logs: Vec<L2ToL1Log>,
    /// Status: either 1 (success) or 0 (failure).
    pub status: Option<U64>,
    /// State root.
    pub root: Option<H256>,
    /// Logs bloom
    // #[serde(rename = "logsBloom")]
    // pub logs_bloom: H2048,
    /// Transaction type
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<U64>,
    // Effective gas price
    // #[serde(rename = "effectiveGasPrice")]
    // pub effective_gas_price: Option<U256>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDetails {
    pub is_l1_originated: bool,
    pub status: TransactionStatus,
    pub fee: U256,
    pub gas_per_pubdata: Option<U256>,
    pub initiator_address: Address,
    pub received_at: DateTime<Utc>,
    pub eth_commit_tx_hash: Option<H256>,
    pub eth_prove_tx_hash: Option<H256>,
    pub eth_execute_tx_hash: Option<H256>,
}

/// A log produced by a transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Log {
    /// H256
    pub address: H256,
    /// Topics
    pub topics: Vec<H256>,
    /// Data
    pub data: Bytes,
    /// Block Hash
    #[serde(rename = "blockHash")]
    pub block_hash: Option<H256>,
    /// Block Number
    #[serde(rename = "blockNumber")]
    pub block_number: Option<U64>,
    /// L1 batch number the log is included in.
    #[serde(rename = "l1BatchNumber")]
    pub l1_batch_number: Option<U64>,
    /// Transaction Hash
    #[serde(rename = "transactionHash")]
    pub transaction_hash: Option<H256>,
    /// Transaction Index
    #[serde(rename = "transactionIndex")]
    pub transaction_index: Option<Index>,
    /// Log Index in Block
    #[serde(rename = "logIndex")]
    pub log_index: Option<U256>,
    /// Log Index in Transaction
    #[serde(rename = "transactionLogIndex")]
    pub transaction_log_index: Option<U256>,
    /// Log Type
    #[serde(rename = "logType")]
    pub log_type: Option<String>,
    /// Removed
    pub removed: Option<bool>,
}

impl Log {
    /// Returns true if the log has been removed.
    pub fn is_removed(&self) -> bool {
        if let Some(val_removed) = self.removed {
            return val_removed;
        }

        if let Some(ref val_log_type) = self.log_type {
            if val_log_type == "removed" {
                return true;
            }
        }
        false
    }
}
