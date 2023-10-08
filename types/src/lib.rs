use serde::{Serialize, Deserialize};

pub use ola_basic_types::*;

pub mod l2;
pub mod tx;
pub mod request;
pub mod api;
pub mod utils;
pub mod storage;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputData {
    pub hash: H256,
    pub data: Vec<u8>,
}