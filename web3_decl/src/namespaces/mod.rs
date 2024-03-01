use jsonrpsee::core::Error;

pub mod eth;
pub mod offchain_verifier;
pub mod ola;

pub type RpcResult<T> = Result<T, Error>;
