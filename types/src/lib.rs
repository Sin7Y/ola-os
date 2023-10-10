use serde::{Deserialize, Serialize};

pub use ola_basic_types::*;

pub mod api;
pub mod l2;
pub mod request;
pub mod storage;
pub mod tx;
pub mod utils;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputData {
    pub hash: H256,
    pub data: Vec<u8>,
}
