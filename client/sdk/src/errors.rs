use ethereum_types::Secret;
pub use ola_web3_decl::jsonrpsee::core::Error as RpcError;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignerError {
    #[error("Invalid private key: {0}")]
    InvalidPrivateKey(Secret),
    #[error("Signing failed: {0}")]
    SigningFailed(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NumberConvertError {
    #[error("H256 to u64 array failed: {0}")]
    H256ToU64ArrayFailed(String),

    #[error("Invalid Ola Hash: {0}")]
    InvalidOlaHash(String),

    #[error("secp error: {0}")]
    SecpError(secp256k1::Error),
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Missing required field for a transaction: {0}")]
    MissingRequiredField(String),
    #[error("Signing error: {0}")]
    SigningError(#[from] SignerError),
    #[error("RPC error: {0:?}")]
    RpcError(#[from] RpcError),
    #[error("Invalid ABI File")]
    AbiParseError,
}
