use ola_contracts::BaseSystemContractsHashes;
use ola_types::{
    api,
    block::{L1BatchHeader, MiniblockHeader},
    commitment::{L1BatchMetaParameters, L1BatchMetadata},
    Address, L1BatchNumber, MiniblockNumber, H256,
};
use sqlx::{postgres::PgArguments, query::Query, Error, Postgres};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageL1BatchConvertError {
    #[error("Incomplete L1 batch")]
    Incomplete,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageL1BatchHeader {
    pub number: i64,
    pub timestamp: i64,
    pub is_finished: bool,
    pub l1_tx_count: i32,
    pub l2_tx_count: i32,
    pub fee_account_address: Vec<u8>,
    // pub priority_ops_onchain_data: Vec<Vec<u8>>,
    pub used_contract_hashes: serde_json::Value,
    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,
    pub protocol_version: Option<i32>,
}

impl From<StorageL1BatchHeader> for L1BatchHeader {
    fn from(l1_batch: StorageL1BatchHeader) -> Self {
        // TODO:
        // let priority_ops_onchain_data: Vec<_> = l1_batch
        //     .priority_ops_onchain_data
        //     .into_iter()
        //     .map(|raw_data| raw_data.into())
        //     .collect();

        L1BatchHeader {
            number: L1BatchNumber(l1_batch.number as u32),
            is_finished: l1_batch.is_finished,
            timestamp: l1_batch.timestamp as u64,
            fee_account_address: Address::from_slice(&l1_batch.fee_account_address),
            // FIXME: use real priority_ops_onchain_data
            priority_ops_onchain_data: vec![],
            l1_tx_count: l1_batch.l1_tx_count as u16,
            l2_tx_count: l1_batch.l2_tx_count as u16,
            // FIXME: use real l2_to_l1_logs and l2_to_l1_messages
            l2_to_l1_logs: vec![],
            l2_to_l1_messages: vec![],

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
pub fn web3_block_where_sql(block_id: api::BlockId, arg_index: u8) -> String {
    match block_id {
        api::BlockId::Hash(_) => format!("miniblocks.hash = ${arg_index}"),
        api::BlockId::Number(api::BlockNumber::Number(_)) => {
            format!("miniblocks.number = ${arg_index}")
        }
        api::BlockId::Number(number) => {
            let block_sql = web3_block_number_to_sql(number);
            format!("miniblocks.number = {}", block_sql)
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

    pub parent_hash: Option<Vec<u8>>,
    pub hash: Option<Vec<u8>>,
    pub merkle_root_hash: Option<Vec<u8>>,
    pub commitment: Option<Vec<u8>>,
    pub meta_parameters_hash: Option<Vec<u8>>,
    pub pass_through_data_hash: Option<Vec<u8>>,
    pub aux_data_hash: Option<Vec<u8>>,

    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,

    pub compressed_initial_writes: Option<Vec<u8>>,
    pub compressed_repeated_writes: Option<Vec<u8>>,
    pub compressed_write_logs: Option<Vec<u8>>,
    pub compressed_contracts: Option<Vec<u8>>,

    pub used_contract_hashes: serde_json::Value,

    pub protocol_version: Option<i32>,

    pub events_queue_commitment: Option<Vec<u8>>,
}

impl From<StorageL1Batch> for L1BatchHeader {
    fn from(l1_batch: StorageL1Batch) -> Self {
        L1BatchHeader {
            number: L1BatchNumber(l1_batch.number as u32),
            is_finished: l1_batch.is_finished,
            timestamp: l1_batch.timestamp as u64,
            fee_account_address: Address::from_slice(&l1_batch.fee_account_address),
            l1_tx_count: l1_batch.l1_tx_count as u16,
            l2_tx_count: l1_batch.l2_tx_count as u16,
            // TODO:
            // FIXME: use real priority_ops_onchain_data,l2_to_l1_logs,l2_to_l1_messages
            priority_ops_onchain_data: vec![],
            l2_to_l1_logs: vec![],
            l2_to_l1_messages: vec![],

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

impl TryInto<L1BatchMetadata> for StorageL1Batch {
    type Error = StorageL1BatchConvertError;

    fn try_into(self) -> Result<L1BatchMetadata, Self::Error> {
        Ok(L1BatchMetadata {
            root_hash: H256::from_slice(&self.hash.ok_or(StorageL1BatchConvertError::Incomplete)?),
            merkle_root_hash: H256::from_slice(
                &self
                    .merkle_root_hash
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
            ),
            initial_writes_compressed: self
                .compressed_initial_writes
                .ok_or(StorageL1BatchConvertError::Incomplete)?,
            repeated_writes_compressed: self
                .compressed_repeated_writes
                .ok_or(StorageL1BatchConvertError::Incomplete)?,
            // TODO: use real rollup_last_leaf_index,l2_l1_messages_compressed,l2_l1_merkle_root
            // TODO: compressed_state_diffs
            rollup_last_leaf_index: 0 as u64,
            l2_l1_messages_compressed: vec![],
            l2_l1_merkle_root: H256::default(),
            state_diffs_compressed: vec![],
            aux_data_hash: H256::from_slice(
                &self
                    .aux_data_hash
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
            ),
            meta_parameters_hash: H256::from_slice(
                &self
                    .meta_parameters_hash
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
            ),
            pass_through_data_hash: H256::from_slice(
                &self
                    .pass_through_data_hash
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
            ),
            commitment: H256::from_slice(
                &self
                    .commitment
                    .ok_or(StorageL1BatchConvertError::Incomplete)?,
            ),
            block_meta_params: L1BatchMetaParameters {
                bootloader_code_hash: H256::from_slice(
                    &self
                        .bootloader_code_hash
                        .ok_or(StorageL1BatchConvertError::Incomplete)?,
                ),
                default_aa_code_hash: H256::from_slice(
                    &self
                        .default_aa_code_hash
                        .ok_or(StorageL1BatchConvertError::Incomplete)?,
                ),
            },
            events_queue_commitment: self.events_queue_commitment.map(|v| H256::from_slice(&v)),
        })
    }
}

// TODO add more fields later.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageL1BatchDetails {
    pub number: i64,
    pub timestamp: i64,
    pub l1_tx_count: i32,
    pub l2_tx_count: i32,
    pub hash: Option<Vec<u8>>,
    // pub commit_tx_hash: Option<String>,
    // pub committed_at: Option<NaiveDateTime>,
    // pub prove_tx_hash: Option<String>,
    // pub proven_at: Option<NaiveDateTime>,
    // pub execute_tx_hash: Option<String>,
    // pub executed_at: Option<NaiveDateTime>,
    // pub l2_fair_gas_price: i64,
    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,
}

impl From<StorageL1BatchDetails> for api::L1BatchDetails {
    fn from(details: StorageL1BatchDetails) -> Self {
        let status = if details.number == 0
        // || details.execute_tx_hash.is_some()
        {
            api::BlockStatus::Verified
        } else {
            api::BlockStatus::Sealed
        };

        let base = api::BlockDetailsBase {
            timestamp: details.timestamp as u64,
            l1_tx_count: details.l1_tx_count as usize,
            l2_tx_count: details.l2_tx_count as usize,
            status,
            root_hash: Some(H256::from_slice(&details.hash.unwrap())),
            commit_tx_hash: None,
            committed_at: None,
            prove_tx_hash: None,
            proven_at: None,
            execute_tx_hash: None,
            executed_at: None,
            offchain_picked_at: None,
            offchain_verified_at: None,
            l1_gas_price: 0,
            l2_fair_gas_price: 0,
            base_system_contracts_hashes: convert_base_system_contracts_hashes(
                details.bootloader_code_hash,
                details.default_aa_code_hash,
            ),
        };
        api::L1BatchDetails {
            base,
            number: L1BatchNumber(details.number as u32),
        }
    }
}

// TODO add more fields later.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StorageBlockDetails {
    pub number: i64,
    pub l1_batch_number: Option<i64>,
    pub hash: Vec<u8>,
    pub l1_tx_count: i32,
    pub l2_tx_count: i32,
    pub timestamp: i64,
    pub bootloader_code_hash: Option<Vec<u8>>,
    pub default_aa_code_hash: Option<Vec<u8>>,
}

impl From<StorageBlockDetails> for api::BlockDetails {
    fn from(details: StorageBlockDetails) -> Self {
        let status = if details.number == 0
        // || details.execute_tx_hash.is_some()
        {
            api::BlockStatus::Verified
        } else {
            api::BlockStatus::Sealed
        };

        let base = api::BlockDetailsBase {
            timestamp: details.timestamp as u64,
            l1_tx_count: details.l1_tx_count as usize,
            l2_tx_count: details.l2_tx_count as usize,
            status,
            root_hash: Some(H256::from_slice(&details.hash)),
            commit_tx_hash: None,
            committed_at: None,
            prove_tx_hash: None,
            proven_at: None,
            execute_tx_hash: None,
            executed_at: None,
            // TODO add offchain verifiercation
            offchain_picked_at: None,
            offchain_verified_at: None,
            l1_gas_price: 0,
            l2_fair_gas_price: 0,
            base_system_contracts_hashes: convert_base_system_contracts_hashes(
                details.bootloader_code_hash,
                details.default_aa_code_hash,
            ),
        };
        api::BlockDetails {
            base,
            number: MiniblockNumber(details.number as u32),
            l1_batch_number: L1BatchNumber(details.l1_batch_number.unwrap() as u32),
            operator_address: Address::default(),
            protocol_version: None,
        }
    }
}
