pub use crate::request::{SerializationTransactionError, TransactionRequest};
use chrono::{DateTime, Utc};
use ola_basic_types::{Address, H256, U256};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use strum::Display;
use web3::types::U64;

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
    Finalized,
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
            BlockNumber::Finalized => serializer.serialize_str("finalized"),
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
                    "finalized" => BlockNumber::Finalized,
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
