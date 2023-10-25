use ola_basic_types::{AccountTreeId, Address, U256};
use ola_utils::u256_to_h256;
use serde::{Deserialize, Serialize};

use crate::{StorageKey, StorageValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageLogQuery {
    pub log_query: LogQuery,
    pub log_type: StorageLogQueryType,
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StorageLogQueryType {
    Read,
    InitialWrite,
    RepeatedWrite,
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
