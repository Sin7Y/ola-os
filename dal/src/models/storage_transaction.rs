use std::str::FromStr;

use bigdecimal::BigDecimal;
use ola_types::{
    api::{TransactionDetails, TransactionStatus},
    l2::{L2TxCommonData, TransactionType},
    protocol_version::ProtocolUpgradeTxCommonData,
    tx::execute::Execute,
    Address, ExecuteTransactionCommon, Nonce, Transaction, EIP_1559_TX_TYPE, EIP_712_TX_TYPE, H256,
    OLA_RAW_TX_TYPE, PROTOCOL_UPGRADE_TX_TYPE, U256,
};
use ola_utils::bigdecimal_to_u256;
use sqlx::types::chrono::{DateTime, NaiveDateTime, Utc};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageTransaction {
    pub priority_op_id: Option<i64>,
    pub hash: Vec<u8>,
    pub is_priority: bool,
    pub initiator_address: Vec<u8>,
    pub nonce: Option<i64>,
    pub signature: Option<Vec<u8>>,
    pub input: Option<Vec<u8>>,
    pub tx_format: Option<i32>,
    pub data: serde_json::Value,
    pub received_at: NaiveDateTime,
    pub in_mempool: bool,

    pub l1_block_number: Option<i32>,
    pub l1_batch_number: Option<i64>,
    pub l1_batch_tx_index: Option<i32>,
    pub miniblock_number: Option<i64>,
    pub index_in_block: Option<i32>,
    pub error: Option<String>,
    pub contract_address: Option<Vec<u8>>,

    pub execution_info: serde_json::Value,
    pub upgrade_id: Option<i32>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<StorageTransaction> for L2TxCommonData {
    fn from(tx: StorageTransaction) -> Self {
        let nonce = Nonce(tx.nonce.expect("no nonce in L2 tx in DB") as u32);

        let StorageTransaction {
            initiator_address,
            signature,
            hash,
            input,
            ..
        } = tx;

        let tx_format = match tx.tx_format.map(|a| a as u8) {
            Some(EIP_712_TX_TYPE) | None => TransactionType::EIP712Transaction,
            Some(EIP_1559_TX_TYPE) => TransactionType::EIP1559Transaction,
            Some(OLA_RAW_TX_TYPE) => TransactionType::OlaRawTransaction,
            Some(_) => unreachable!("Unsupported tx type"),
        };

        L2TxCommonData::new(
            nonce,
            Address::from_slice(&initiator_address),
            signature.unwrap_or_else(|| {
                panic!("Signature is mandatory for transactions. Tx {:#?}", hash)
            }),
            tx_format,
            input.expect("input data is mandatory for l2 transactions"),
            H256::from_slice(&hash),
        )
    }
}

impl From<StorageTransaction> for ProtocolUpgradeTxCommonData {
    fn from(tx: StorageTransaction) -> Self {
        let canonical_tx_hash = H256::from_slice(&tx.hash);

        ProtocolUpgradeTxCommonData {
            sender: Address::from_slice(&tx.initiator_address),
            upgrade_id: (tx.upgrade_id.unwrap() as u16).try_into().unwrap(),
            eth_hash: Default::default(),
            eth_block: tx.l1_block_number.unwrap_or_default() as u64,
            canonical_tx_hash,
        }
    }
}

impl From<StorageTransaction> for Transaction {
    fn from(tx: StorageTransaction) -> Self {
        let hash = H256::from_slice(&tx.hash);
        let execute = serde_json::from_value::<Execute>(tx.data.clone())
            .unwrap_or_else(|_| panic!("invalid json in database for tx {:?}", hash));
        let received_timestamp_ms = tx.received_at.timestamp_millis() as u64;
        match tx.tx_format {
            Some(t) if t == PROTOCOL_UPGRADE_TX_TYPE as i32 => Transaction {
                common_data: ExecuteTransactionCommon::ProtocolUpgrade(tx.into()),
                execute,
                received_timestamp_ms,
            },
            _ => Transaction {
                common_data: ExecuteTransactionCommon::L2(tx.into()),
                execute,
                received_timestamp_ms,
            },
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageTransactionDetails {
    pub is_priority: bool,
    pub initiator_address: Vec<u8>,
    pub received_at: NaiveDateTime,
    pub miniblock_number: Option<i64>,
    pub error: Option<String>,
}

impl StorageTransactionDetails {
    fn get_transaction_status(&self) -> TransactionStatus {
        if self.error.is_some() {
            TransactionStatus::Failed
        } else if self.miniblock_number.is_some() {
            TransactionStatus::Included
        } else {
            TransactionStatus::Pending
        }
    }
}

impl From<StorageTransactionDetails> for TransactionDetails {
    fn from(tx_details: StorageTransactionDetails) -> Self {
        let status = tx_details.get_transaction_status();

        let initiator_address = H256::from_slice(tx_details.initiator_address.as_slice());
        let received_at = DateTime::<Utc>::from_naive_utc_and_offset(tx_details.received_at, Utc);

        TransactionDetails {
            is_l1_originated: tx_details.is_priority,
            status,
            fee: U256::default(),
            gas_per_pubdata: None,
            initiator_address,
            received_at,
            eth_commit_tx_hash: None,
            eth_prove_tx_hash: None,
            eth_execute_tx_hash: None,
        }
    }
}
