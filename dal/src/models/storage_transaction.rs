use ola_types::api::TransactionReceipt;
use ola_types::tx::primitives::PackedEthSignature;
use ola_types::{
    api,
    api::{TransactionDetails, TransactionStatus},
    l2::{L2TxCommonData, TransactionType},
    protocol_version::ProtocolUpgradeTxCommonData,
    tx::execute::Execute,
    Address, Bytes, ExecuteTransactionCommon, L2ChainId, Nonce, Transaction, EIP_1559_TX_TYPE,
    EIP_712_TX_TYPE, H2048, H256, OLA_RAW_TX_TYPE, PROTOCOL_UPGRADE_TX_TYPE, U256, U64,
};

use ola_utils::{bigdecimal_to_u256, h256_to_account_address};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::types::chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{Error, FromRow, Row};

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

#[derive(sqlx::FromRow)]
pub(crate) struct StorageTransactionReceipt {
    pub error: Option<String>,
    pub tx_format: Option<i32>,
    pub index_in_block: Option<i32>,
    pub block_hash: Vec<u8>,
    pub tx_hash: Vec<u8>,
    pub block_number: i64,
    pub l1_batch_tx_index: Option<i32>,
    pub l1_batch_number: Option<i64>,
    pub transfer_to: Option<serde_json::Value>,
    pub execute_contract_address: Option<serde_json::Value>,
    pub contract_address: Option<Vec<u8>>,
    pub initiator_address: Vec<u8>,
}

impl From<StorageTransactionReceipt> for TransactionReceipt {
    fn from(storage_receipt: StorageTransactionReceipt) -> Self {
        let status = storage_receipt.error.map_or_else(U64::one, |_| U64::zero());

        let tx_type = storage_receipt
            .tx_format
            .map_or_else(Default::default, U64::from);
        let transaction_index = storage_receipt
            .index_in_block
            .map_or_else(Default::default, U64::from);

        let block_hash = H256::from_slice(&storage_receipt.block_hash);
        TransactionReceipt {
            transaction_hash: H256::from_slice(&storage_receipt.tx_hash),
            transaction_index,
            block_hash: Some(block_hash),
            block_number: Some(storage_receipt.block_number.into()),
            l1_batch_tx_index: storage_receipt.l1_batch_tx_index.map(U64::from),
            l1_batch_number: storage_receipt.l1_batch_number.map(U64::from),
            from: H256::from_slice(&storage_receipt.initiator_address),
            to: storage_receipt
                .transfer_to
                .or(storage_receipt.execute_contract_address)
                .map(|addr| {
                    serde_json::from_value::<Address>(addr)
                        .expect("invalid address value in the database")
                })
                // For better compatibility with various clients, we never return null.
                .or_else(|| Some(Address::default())),
            gas_used: None,
            effective_gas_price: None,
            logs_bloom: H2048::default(),
            cumulative_gas_used: U256::default(),

            contract_address: storage_receipt
                .contract_address
                .map(|addr| h256_to_account_address(&H256::from_slice(&addr))),
            logs: vec![],
            status: Some(status),
            root: Some(block_hash),
            // Even though the Rust SDK recommends us to supply "None" for legacy transactions
            // we always supply some number anyway to have the same behavior as most popular RPCs
            transaction_type: Some(tx_type),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct StorageApiTransaction {
    #[serde(flatten)]
    pub inner_api_transaction: api::Transaction,
}

impl From<StorageApiTransaction> for api::Transaction {
    fn from(tx: StorageApiTransaction) -> Self {
        tx.inner_api_transaction
    }
}
impl<'r> FromRow<'r, PgRow> for StorageApiTransaction {
    fn from_row(db_row: &'r PgRow) -> Result<Self, Error> {
        let row_signature: Option<Vec<u8>> = db_row.get("signature");
        let signature = row_signature
            .and_then(|signature| PackedEthSignature::deserialize_packed(&signature).ok());

        // TODO !!!!!!
        Ok(StorageApiTransaction {
            inner_api_transaction: api::Transaction {
                hash: H256::from_slice(db_row.get("tx_hash")),
                nonce: U256::from(db_row.try_get::<i64, &str>("nonce").ok().unwrap_or(0)),
                block_hash: db_row.try_get("block_hash").ok().map(H256::from_slice),
                block_number: db_row
                    .try_get::<i64, &str>("block_number")
                    .ok()
                    .map(U64::from),
                transaction_index: db_row
                    .try_get::<i32, &str>("index_in_block")
                    .ok()
                    .map(U64::from),
                from: Some(H256::from_slice(db_row.get("initiator_address"))),
                to: Some(
                    serde_json::from_value::<Address>(db_row.get("execute_contract_address"))
                        .expect("incorrect address value in the database"),
                ),
                value: U256::default(),
                // `gas_price`, `max_fee_per_gas`, `max_priority_fee_per_gas` will be zero for the priority transactions.
                // For common L2 transactions `gas_price` is equal to `effective_gas_price` if the transaction is included
                // in some block, or `max_fee_per_gas` otherwise.
                gas_price: None,
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                gas: U256::default(),
                input: Bytes::default(),
                raw: None,
                v: signature.as_ref().map(|s| U64::from(s.v())),
                r: signature.as_ref().map(|s| U256::from(s.r())),
                s: signature.as_ref().map(|s| U256::from(s.s())),
                transaction_type: db_row
                    .try_get::<Option<i32>, &str>("tx_format")
                    .unwrap_or_default()
                    .map(U64::from),
                access_list: None,
                chain_id: U256::zero(),
                l1_batch_number: db_row
                    .try_get::<i64, &str>("l1_batch_number_tx")
                    .ok()
                    .map(U64::from),
                l1_batch_tx_index: db_row
                    .try_get::<i32, &str>("l1_batch_tx_index")
                    .ok()
                    .map(U64::from),
            },
        })
    }
}

pub fn web3_transaction_select_sql() -> &'static str {
    r#"
         transactions.hash as tx_hash,
         transactions.index_in_block as index_in_block,
         transactions.miniblock_number as block_number,
         transactions.nonce as nonce,
         transactions.signature as signature,
         transactions.initiator_address as initiator_address,
         transactions.tx_format as tx_format,
         transactions.l1_batch_number as l1_batch_number_tx,
         transactions.l1_batch_tx_index as l1_batch_tx_index,
         transactions.data->'contractAddress' as "execute_contract_address",
         transactions.data->'calldata' as "calldata",
         miniblocks.hash as "block_hash"
    "#
}

pub fn extract_web3_transaction(db_row: PgRow, chain_id: L2ChainId) -> api::Transaction {
    let mut storage_api_tx = StorageApiTransaction::from_row(&db_row).unwrap();
    storage_api_tx.inner_api_transaction.chain_id = U256::from(chain_id.0);
    if storage_api_tx.inner_api_transaction.transaction_type == Some(U64::from(0)) {
        storage_api_tx.inner_api_transaction.v = storage_api_tx
            .inner_api_transaction
            .v
            .map(|v| v + 35 + chain_id.0 * 2);
    }
    storage_api_tx.into()
}
