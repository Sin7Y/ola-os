use ola_basic_types::{AccountTreeId, Address, U256, H256};
use ola_utils::u256_to_h256;
use serde::{Deserialize, Serialize};

use crate::{StorageKey, StorageValue};
use olavm_exe_core::{
    merkle_tree::log::{WitnessStorageLog as OlavmWitnessStorageLog, StorageLog as OlavmStorageLog, StorageLogKind as OlavmStorageLogKind},
    types::{
        account::AccountTreeId as OlavmAccountTreeId,
        merkle_tree::{TreeKey as OlavmTreeKey, TreeValue as OlavmTreeValue, h256_to_tree_key, h256_to_tree_value},
        storage::StorageKey as OlavmStorageKey,
    }
};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WitnessStorageLog {
    pub storage_log: StorageLog,
    pub previous_value: H256,
}

impl WitnessStorageLog {
    pub fn to_olavm_type(&self) -> OlavmWitnessStorageLog {
        OlavmWitnessStorageLog {
            storage_log: OlavmStorageLog {
                 kind: if self.storage_log.kind == StorageLogKind::Read {
                    OlavmStorageLogKind::Read
                 } else {
                    OlavmStorageLogKind::Write
                 },
                key: h256_to_tree_key(&self.storage_log.key.hashed_key()),
                value: h256_to_tree_value(&self.storage_log.value),
            },
            previous_value: h256_to_tree_value(&self.previous_value),
        }
    }
}