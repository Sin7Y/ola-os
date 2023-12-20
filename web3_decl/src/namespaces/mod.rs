use jsonrpsee::core::Error;

pub mod ola;
pub mod eth;

pub type RpcResult<T> = Result<T, Error>;
