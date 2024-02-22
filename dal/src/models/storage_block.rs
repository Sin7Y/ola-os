use ola_contracts::BaseSystemContractsHashes;
use ola_types::{
    api,
    block::{L1BatchHeader, MiniblockHeader},
    Address, L1BatchNumber, MiniblockNumber, H256,
};
use sqlx::{postgres::PgArguments, query::Query, types::BigDecimal, Postgres};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageL1BatchHeader {
    pub number: i64,
    pub timestamp: i64,
    pub is_finished: bool,
    pub l1_tx_count: i32,
    pub l2_tx_count: i32,
    pub used_contract_hashes: serde_json::Value,
    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,
    pub protocol_version: Option<i32>,
}

impl From<StorageL1BatchHeader> for L1BatchHeader {
    fn from(l1_batch: StorageL1BatchHeader) -> Self {
        L1BatchHeader {
            number: L1BatchNumber(l1_batch.number as u32),
            is_finished: l1_batch.is_finished,
            timestamp: l1_batch.timestamp as u64,
            l1_tx_count: l1_batch.l1_tx_count as u16,
            l2_tx_count: l1_batch.l2_tx_count as u16,

            used_contract_hashes: serde_json::from_value(l1_batch.used_contract_hashes)
                .expect("invalid value for used_contract_hashes in the DB"),
            base_system_contracts_hashes: convert_base_system_contracts_hashes(
                l1_batch.bootloader_code_hash,
                l1_batch.default_aa_code_hash,
            ),
            protocol_version: l1_batch
                .protocol_version
                .map(|v| (v as u16).try_into().unwrap()),
        }
    }
}

pub struct StorageMiniblockHeader {
    pub number: i64,
    pub timestamp: i64,
    pub hash: Vec<u8>,
    pub l1_tx_count: i32,
    pub l2_tx_count: i32,
    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,
    pub protocol_version: Option<i32>,
}

impl From<StorageMiniblockHeader> for MiniblockHeader {
    fn from(row: StorageMiniblockHeader) -> Self {
        MiniblockHeader {
            number: MiniblockNumber(row.number as u32),
            timestamp: row.timestamp as u64,
            hash: H256::from_slice(&row.hash),
            l1_tx_count: row.l1_tx_count as u16,
            l2_tx_count: row.l2_tx_count as u16,
            base_system_contracts_hashes: convert_base_system_contracts_hashes(
                row.bootloader_code_hash,
                row.default_aa_code_hash,
            ),
            protocol_version: row.protocol_version.map(|v| (v as u16).try_into().unwrap()),
        }
    }
}

pub fn web3_block_number_to_sql(block_number: api::BlockNumber) -> String {
    match block_number {
        api::BlockNumber::Number(number) => number.to_string(),
        api::BlockNumber::Earliest => 0.to_string(),
        api::BlockNumber::Pending => {
            "(SELECT (MAX(number) + 1) as number FROM miniblocks)".to_string()
        }
        api::BlockNumber::Latest | api::BlockNumber::Committed => {
            "(SELECT MAX(number) as number FROM miniblocks)".to_string()
        }
    }
}

pub fn bind_block_where_sql_params<'q>(
    block_id: &'q api::BlockId,
    query: Query<'q, Postgres, PgArguments>,
) -> Query<'q, Postgres, PgArguments> {
    match block_id {
        // these block_id types result in `$1` in the query string, which we have to `bind`
        api::BlockId::Hash(block_hash) => query.bind(block_hash.as_bytes()),
        api::BlockId::Number(api::BlockNumber::Number(number)) => {
            query.bind(number.as_u64() as i64)
        }
        // others don't introduce `$1`, so we don't have to `bind` anything
        _ => query,
    }
}

fn convert_base_system_contracts_hashes(
    entrypoint_code_hash: Option<Vec<u8>>,
    default_aa_code_hash: Option<Vec<u8>>,
) -> BaseSystemContractsHashes {
    BaseSystemContractsHashes {
        entrypoint: entrypoint_code_hash
            .map(|hash| H256::from_slice(&hash))
            .expect("should not be none"),
        default_aa: default_aa_code_hash
            .map(|hash| H256::from_slice(&hash))
            .expect("should not be none"),
    }
}

/// Information about L1 batch which a certain miniblock belongs to.
#[derive(Debug)]
pub struct ResolvedL1BatchForMiniblock {
    /// L1 batch which the miniblock belongs to. `None` if the miniblock is not explicitly attached
    /// (i.e., its L1 batch is not sealed).
    pub miniblock_l1_batch: Option<L1BatchNumber>,
    /// Pending (i.e., unsealed) L1 batch.
    pub pending_l1_batch: L1BatchNumber,
}

impl ResolvedL1BatchForMiniblock {
    /// Returns the L1 batch number that the miniblock has now or will have in the future (provided
    /// that the node will operate correctly).
    pub fn expected_l1_batch(&self) -> L1BatchNumber {
        self.miniblock_l1_batch.unwrap_or(self.pending_l1_batch)
    }
}

/// Projection of the columns corresponding to [`L1BatchHeader`] + [`L1BatchMetadata`].
// TODO(PLA-369): use `#[sqlx(flatten)]` once upgraded to newer `sqlx`
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageL1Batch {
    pub number: i64,
    pub timestamp: i64,
    pub is_finished: bool,
    pub l1_tx_count: i32,
    pub l2_tx_count: i32,
    pub fee_account_address: Vec<u8>,
    pub bloom: Vec<u8>,
    pub l2_to_l1_logs: Vec<Vec<u8>>,
    pub priority_ops_onchain_data: Vec<Vec<u8>>,

    pub parent_hash: Option<Vec<u8>>,
    pub hash: Option<Vec<u8>>,
    pub merkle_root_hash: Option<Vec<u8>>,
    pub commitment: Option<Vec<u8>>,
    pub meta_parameters_hash: Option<Vec<u8>>,
    pub pass_through_data_hash: Option<Vec<u8>>,
    pub aux_data_hash: Option<Vec<u8>>,

    pub rollup_last_leaf_index: Option<i64>,
    pub zkporter_is_available: Option<bool>,
    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,

    pub l2_to_l1_messages: Vec<Vec<u8>>,
    pub l2_l1_compressed_messages: Option<Vec<u8>>,
    pub l2_l1_merkle_root: Option<Vec<u8>>,
    pub compressed_initial_writes: Option<Vec<u8>>,
    pub compressed_repeated_writes: Option<Vec<u8>>,
    pub compressed_write_logs: Option<Vec<u8>>,
    pub compressed_contracts: Option<Vec<u8>>,

    pub eth_prove_tx_id: Option<i32>,
    pub eth_commit_tx_id: Option<i32>,
    pub eth_execute_tx_id: Option<i32>,

    pub used_contract_hashes: serde_json::Value,

    pub base_fee_per_gas: BigDecimal,
    pub l1_gas_price: i64,
    pub l2_fair_gas_price: i64,

    pub system_logs: Vec<Vec<u8>>,
    pub compressed_state_diffs: Option<Vec<u8>>,

    pub protocol_version: Option<i32>,

    pub events_queue_commitment: Option<Vec<u8>>,
    pub bootloader_initial_content_commitment: Option<Vec<u8>>,
    pub pubdata_input: Option<Vec<u8>>,
}

// impl From<StorageL1Batch> for L1BatchHeader {
// fn from(l1_batch: StorageL1Batch) -> Self {
//     let priority_ops_onchain_data: Vec<_> = l1_batch
//         .priority_ops_onchain_data
//         .into_iter()
//         .map(Vec::into)
//         .collect();

//     L1BatchHeader {
//         number: L1BatchNumber(l1_batch.number as u32),
//         is_finished: l1_batch.is_finished,
//         timestamp: l1_batch.timestamp as u64,
//         fee_account_address: Address::from_slice(&l1_batch.fee_account_address),
//         l1_tx_count: l1_batch.l1_tx_count as u16,
//         l2_tx_count: l1_batch.l2_tx_count as u16,

//         used_contract_hashes: serde_json::from_value(l1_batch.used_contract_hashes)
//             .expect("invalid value for used_contract_hashes in the DB"),
//         base_system_contracts_hashes: convert_base_system_contracts_hashes(
//             l1_batch.bootloader_code_hash,
//             l1_batch.default_aa_code_hash,
//         ),
//         protocol_version: l1_batch
//             .protocol_version
//             .map(|v| (v as u16).try_into().unwrap()),
//     }
// }
// }
