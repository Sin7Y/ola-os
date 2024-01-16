pub use ola_basic_types::{AccountTreeId, Address, L2ChainId, H256, U256};
use ola_config::constants::contracts::{
    ACCOUNT_CODE_STORAGE_ADDRESS, KNOWN_CODES_STORAGE_ADDRESS, NONCE_HOLDER_ADDRESS,
    SYSTEM_CONTEXT_ADDRESS,
};
use ola_utils::{
    convert::address_to_h256,
    hash::{hash_bytes, PoseidonBytes},
};

use olavm_plonky2::hash::utils::h256_add_offset;
use serde::{Deserialize, Serialize};

pub mod log;
pub mod witness_block_state;
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

    pub fn raw_hashed_key(address: &H256, key: &H256) -> [u8; 32] {
        let mut bytes = [0_u8; 64];
        bytes[0..32].copy_from_slice(&address.0);
        U256::from(key.to_fixed_bytes()).to_big_endian(&mut bytes[32..64]);
        bytes.hash_bytes()
    }

    pub fn hashed_key(&self) -> H256 {
        Self::raw_hashed_key(self.address(), self.key()).into()
    }

    pub fn hashed_key_u256(&self) -> U256 {
        U256::from_little_endian(&Self::raw_hashed_key(self.address(), self.key()))
    }

    pub fn add(&self, val: u64) -> Self {
        let bytes = self.key().as_bytes().to_vec();
        let bytes: [u8; 32] = bytes.try_into().unwrap();
        let key = h256_add_offset(bytes, val);
        Self::new(self.account, H256(key))
    }
}

pub type StorageValue = H256;

fn get_address_mapping_key(position: H256, address: &Address) -> H256 {
    let padded_address = address_to_h256(address);
    hash_bytes(&[position.as_bytes(), padded_address.as_bytes()].concat())
}

pub fn get_nonce_key(account: &Address) -> StorageKey {
    let nonce_manager = AccountTreeId::new(NONCE_HOLDER_ADDRESS);

    // The `minNonce` (used as nonce for EOAs) is stored in a mapping inside the NONCE_HOLDER system contract
    let key = get_address_mapping_key(H256::zero(), account);

    StorageKey::new(nonce_manager, key)
}

pub fn get_full_code_key(account: &Address) -> StorageKey {
    let account_code_storage = AccountTreeId::new(ACCOUNT_CODE_STORAGE_ADDRESS);
    let key = [[0; 32], account.to_fixed_bytes()].concat();
    StorageKey::new(account_code_storage, hash_bytes(&key))
}

pub fn get_known_code_key(hash: &H256) -> StorageKey {
    let known_codes_storage = AccountTreeId::new(KNOWN_CODES_STORAGE_ADDRESS);
    StorageKey::new(known_codes_storage, *hash)
}

pub fn get_system_context_key(key: H256) -> StorageKey {
    let system_context = AccountTreeId::new(SYSTEM_CONTEXT_ADDRESS);
    StorageKey::new(system_context, key)
}
