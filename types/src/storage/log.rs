use ola_basic_types::{AccountTreeId, Address, H256, U256};
use ola_utils::{u256_to_h256, u64_array_to_h256, u64s_to_u256};
use serde::{Deserialize, Serialize};

use crate::{StorageKey, StorageValue};
use olavm_core::vm::hardware::{StorageAccessKind, StorageAccessLog};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageLogQuery {
    pub log_query: LogQuery,
    pub log_type: StorageLogQueryType,
}

impl From<&StorageAccessLog> for StorageLogQuery {
    fn from(log: &StorageAccessLog) -> Self {
        Self {
            log_query: LogQuery::from(log),
            log_type: log.kind.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LogQuery {
    pub timestamp: Timestamp,
    pub tx_number_in_block: u16,
    pub aux_byte: u8,
    pub shard_id: u8,
    pub address: Address,
    pub key: U256,
    pub read_value: U256,
    pub written_value: U256,
    pub rw_flag: bool,
    pub rollback: bool,
    pub is_service: bool,
}

impl From<&StorageAccessLog> for LogQuery {
    fn from(log: &StorageAccessLog) -> Self {
        Self {
            timestamp: Timestamp(log.block_timestamp as u32),
            tx_number_in_block: 0,
            aux_byte: 0,
            shard_id: 0,
            address: u64_array_to_h256(&log.contract_addr),
            key: u64s_to_u256(&log.storage_key),
            read_value: u64s_to_u256(&log.pre_value.unwrap_or_default()),
            written_value: u64s_to_u256(&log.value.unwrap_or_default()),
            rw_flag: log.kind != StorageAccessKind::Read,
            rollback: false,
            is_service: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StorageLogQueryType {
    Read,
    InitialWrite,
    RepeatedWrite,
}

impl From<StorageAccessKind> for StorageLogQueryType {
    fn from(kind: StorageAccessKind) -> Self {
        match kind {
            StorageAccessKind::Read => StorageLogQueryType::Read,
            StorageAccessKind::InitialWrite => StorageLogQueryType::InitialWrite,
            StorageAccessKind::RepeatedWrite => StorageLogQueryType::RepeatedWrite,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Timestamp(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StorageLogKind {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StorageLog {
    pub kind: StorageLogKind,
    pub key: StorageKey,
    pub value: StorageValue,
}

impl StorageLog {
    pub fn from_log_query(log: &StorageLogQuery) -> Self {
        let key = StorageKey::new(
            AccountTreeId::new(log.log_query.address),
            u256_to_h256(log.log_query.key),
        );
        if log.log_query.rw_flag {
            if log.log_query.rollback {
                Self::new_write_log(key, u256_to_h256(log.log_query.read_value))
            } else {
                Self::new_write_log(key, u256_to_h256(log.log_query.written_value))
            }
        } else {
            Self::new_read_log(key, u256_to_h256(log.log_query.read_value))
        }
    }

    pub fn new_read_log(key: StorageKey, value: StorageValue) -> Self {
        Self {
            kind: StorageLogKind::Read,
            key,
            value,
        }
    }

    pub fn new_write_log(key: StorageKey, value: StorageValue) -> Self {
        Self {
            kind: StorageLogKind::Write,
            key,
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WitnessStorageLog {
    pub storage_log: StorageLog,
    pub previous_value: H256,
}
