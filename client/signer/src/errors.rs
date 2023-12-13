use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignerError {
    #[error("Missing private key")]
    MissingPrivateKey,
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
