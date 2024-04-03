use jsonrpsee::types::{error::ErrorCode, ErrorObjectOwned};
use ola_web3_decl::error::Web3Error;

pub mod batch_limiter_middleware;
pub mod error;
pub mod namespaces;

pub fn into_rpc_error(err: Web3Error) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        match err {
            Web3Error::InternalError => ErrorCode::InternalError.code(),
            Web3Error::NoBlock | Web3Error::InvalidChainId(_) => ErrorCode::InvalidParams.code(),
            Web3Error::SerializationError(_) | Web3Error::SubmitTransactionError(_, _) => 3,
            _ => err.to_string(),
        },
        match err {
            Web3Error::SubmitTransactionError(ref msg, _) => msg.clone(),
            _ => err.to_string(),
        },
        match err {
            Web3Error::SubmitTransactionError(_, data) => Some(format!("0x{}", hex::encode(data))),
            _ => None,
        },
    )
}
