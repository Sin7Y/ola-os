use blake2::{Blake2s256, Digest};
use ola_basic_types::{AccountTreeId, Address, H160, H256, U256};
use serde::{Deserialize, Serialize};

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
