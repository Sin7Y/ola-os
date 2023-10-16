use jsonrpsee::core::Error;

pub mod ola;

pub type RpcResult<T> = Result<T, Error>;
