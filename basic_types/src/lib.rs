use serde::{Serialize, Deserialize};
use std::convert::Infallible;
use std::ops::{Add, Deref, DerefMut, Sub};
use std::str::FromStr;
use std::num::ParseIntError;
use std::fmt;

pub use web3::types::{ Address, Bytes, H256, U256, H160 };

#[macro_use]
mod macros;

pub mod network;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord)]
pub struct AccountTreeId {
    address: Address,
}

impl AccountTreeId {
    pub fn new(address: Address) -> Self {
        Self { address }
    }

    pub fn address(&self) -> &Address {
        &self.address
    }

    pub fn to_fixed_bytes(&self) -> [u8; 20] {
        let mut result = [0_u8; 20];
        result.copy_from_slice(&self.address.to_fixed_bytes());
        result
    }

    pub fn from_fixed_bytes(value: [u8; 20]) -> Self {
        let address = Address::from_slice(&value);
        Self {
            address
        }
    }
}

impl Default for AccountTreeId {
    fn default() -> Self {
        Self {
            address: Address::zero(),
        }
    }
}

impl Into<U256> for AccountTreeId {
    fn into(self) -> U256 {
        let mut be_data = [0_u8; 32];
        be_data[12..].copy_from_slice(&self.to_fixed_bytes());
        U256::from_big_endian(&be_data)
    }
}

impl TryFrom<U256> for AccountTreeId {
    type Error = Infallible;

    fn try_from(value: U256) -> Result<Self, Infallible> {
        let mut be_data = vec![0; 32];
        value.to_big_endian(&mut be_data);
        Ok(Self::from_fixed_bytes(be_data[12..].try_into().unwrap()))
    }
}

basic_type!(
    MiniblockNumber,
    u32
);

basic_type!(
    L1BatchNumber,
    u32
);

basic_type!(
    L1BlockNumber,
    u32
);

basic_type!(
    Nonce,
    u32
);

basic_type!(
    L1ChainId,
    u64
);

basic_type!(
    L2ChainId,
    u16
);