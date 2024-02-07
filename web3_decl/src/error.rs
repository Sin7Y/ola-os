use ola_types::api::SerializationTransactionError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Web3Error {
    #[error("Block with such an ID doesn't exist yet")]
    NoBlock,
    #[error("{0}")]
    SubmitTransactionError(String, Vec<u8>),
    #[error("Failed to serialize transaction: {0}")]
    SerializationError(#[from] SerializationTransactionError),
    #[error("Internal error")]
    InternalError,
    #[error("Invalid l2 chainId `{0}`")]
    InvalidChainId(u16),
}
