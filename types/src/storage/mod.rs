use blake2::{Blake2s256, Digest};
pub use ola_basic_types::{AccountTreeId, Address, L2ChainId, H160, H256, U256};
use ola_config::constants::{
    contracts::{
        ACCOUNT_CODE_STORAGE_ADDRESS, BOOTLOADER_ADDRESS, KNOWN_CODES_STORAGE_ADDRESS,
        SYSTEM_CONTEXT_ADDRESS,
    },
    system_context::{
        SYSTEM_CONTEXT_CHAIN_ID_POSITION, SYSTEM_CONTEXT_COINBASE_POSITION,
        SYSTEM_CONTEXT_DIFFICULTY, SYSTEM_CONTEXT_DIFFICULTY_POSITION,
    },
    NONCE_HOLDER_ADDRESS,
};
use ola_utils::{convert::address_to_h256, hash::hash_bytes};
use olavm_core::{
    types::account::{AccountTreeId as OlavmAccountTreeId, Address as OlavmAddress},
    types::merkle_tree::{h256_to_tree_key, TreeKey as OlavmTreeKey, TreeValue as OlavmTreeValue},
    // merkle_tree::log::{WitnessStorageLog as OlavmWitnessStorageLog, StorageLog as OlavmStorageLog, StorageLogKind as OlavmStorageLogKind},
    // types::merkle_tree::{TreeKey, TreeValue},
    types::storage::StorageKey as OlavmStorageKey,
};
use serde::{Deserialize, Serialize};

use crate::log::StorageLog;

pub mod log;
pub mod writes;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StorageKey {
    account: AccountTreeId,
    key: H256,
}

impl StorageKey {
    pub fn new(account: AccountTreeId, key: H256) -> Self {
        Self { account, key }
    }

    pub fn account(&self) -> &AccountTreeId {
        &self.account
    }

    pub fn key(&self) -> &H256 {
        &self.key
    }

    pub fn address(&self) -> &Address {
        self.account.address()
    }

    pub fn raw_hashed_key(address: &H160, key: &H256) -> [u8; 32] {
        let mut bytes = [0_u8; 64];
        bytes[12..32].copy_from_slice(&address.0);
        U256::from(key.to_fixed_bytes()).to_big_endian(&mut bytes[32..64]);
        Blake2s256::digest(bytes).into()
    }

    pub fn hashed_key(&self) -> H256 {
        Self::raw_hashed_key(self.address(), self.key()).into()
    }

    pub fn hashed_key_u256(&self) -> U256 {
        U256::from_little_endian(&Self::raw_hashed_key(self.address(), self.key()))
    }
}

pub type StorageValue = H256;

fn get_address_mapping_key(address: &Address, position: H256) -> H256 {
    let padded_address = address_to_h256(address);
    hash_bytes(&[padded_address.as_bytes(), position.as_bytes()].concat())
}

pub fn get_nonce_key(account: &Address) -> StorageKey {
    let nonce_manager = AccountTreeId::new(NONCE_HOLDER_ADDRESS);

    // The `minNonce` (used as nonce for EOAs) is stored in a mapping inside the NONCE_HOLDER system contract
    let key = get_address_mapping_key(account, H256::zero());

    StorageKey::new(nonce_manager, key)
}

pub fn get_code_key(account: &Address) -> StorageKey {
    let account_code_storage = AccountTreeId::new(ACCOUNT_CODE_STORAGE_ADDRESS);
    StorageKey::new(account_code_storage, address_to_h256(account))
}

pub fn get_known_code_key(hash: &H256) -> StorageKey {
    let known_codes_storage = AccountTreeId::new(KNOWN_CODES_STORAGE_ADDRESS);
    StorageKey::new(known_codes_storage, *hash)
}

pub fn get_system_context_init_logs(chain_id: L2ChainId) -> Vec<StorageLog> {
    vec![
        StorageLog::new_write_log(
            get_system_context_key(SYSTEM_CONTEXT_CHAIN_ID_POSITION),
            H256::from_low_u64_be(chain_id.0 as u64),
        ),
        StorageLog::new_write_log(
            get_system_context_key(SYSTEM_CONTEXT_COINBASE_POSITION),
            address_to_h256(&BOOTLOADER_ADDRESS),
        ),
        StorageLog::new_write_log(
            get_system_context_key(SYSTEM_CONTEXT_DIFFICULTY_POSITION),
            SYSTEM_CONTEXT_DIFFICULTY,
        ),
    ]
}

pub fn get_system_context_key(key: H256) -> StorageKey {
    let system_context = AccountTreeId::new(SYSTEM_CONTEXT_ADDRESS);
    StorageKey::new(system_context, key)
}
