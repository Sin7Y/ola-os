use ola_basic_types::H256;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Error)]
pub enum TxCheckError {
    #[error("transaction type {0} not supported")]
    UnsupportedType(u32),
    #[error("known transaction. transaction with hash {0} is already in the system")]
    TxDuplication(H256),
}
