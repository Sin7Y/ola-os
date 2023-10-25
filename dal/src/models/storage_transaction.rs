use bigdecimal::BigDecimal;
use sqlx::types::chrono::NaiveDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageTransaction {
    pub priority_op_id: Option<i64>,
    pub hash: Vec<u8>,
    pub is_priority: bool,
    pub full_fee: Option<BigDecimal>,
    pub layer_2_tip_fee: Option<BigDecimal>,
    pub initiator_address: Vec<u8>,
    pub nonce: Option<i64>,
    pub signature: Option<Vec<u8>>,
    pub gas_limit: Option<BigDecimal>,
    pub max_fee_per_gas: Option<BigDecimal>,
    pub max_priority_fee_per_gas: Option<BigDecimal>,
    pub gas_per_storage_limit: Option<BigDecimal>,
    pub gas_per_pubdata_limit: Option<BigDecimal>,
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
    pub effective_gas_price: Option<BigDecimal>,
    pub contract_address: Option<Vec<u8>>,
    pub value: BigDecimal,

    pub paymaster: Vec<u8>,
    pub paymaster_input: Vec<u8>,

    pub refunded_gas: i64,

    pub execution_info: serde_json::Value,

    pub l1_tx_mint: Option<BigDecimal>,
    pub l1_tx_refund_recipient: Option<Vec<u8>>,

    pub upgrade_id: Option<i32>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
