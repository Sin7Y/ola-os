use ola_basic_types::{H256, U256};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InitialStorageWrite {
    pub key: U256,
    pub value: H256,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default, Eq, PartialEq)]
pub struct RepeatedStorageWrite {
    pub index: u64,
    pub value: H256,
}
