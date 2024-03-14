use ola_basic_types::{Address, H256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Eq)]
pub struct L2ToL1Log {
    pub shard_id: u8,
    pub is_service: bool,
    pub tx_number_in_block: u16,
    pub sender: Address,
    pub key: H256,
    pub value: H256,
}

/// A struct representing a "user" L2->L1 log, i.e. the one that has been emitted by using the L1Messenger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Eq)]
pub struct UserL2ToL1Log(pub L2ToL1Log);
