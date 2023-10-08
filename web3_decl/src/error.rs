use thiserror::Error;
use ola_types::api::SerializationTransactionError;

#[derive(Debug, Error)]
pub enum Web3Error {
    #[error("{0}")]
    SubmitTransactionError(String, Vec<u8>),
    #[error("Failed to serialize transaction: {0}")]
    SerializationError(#[from] SerializationTransactionError),
}