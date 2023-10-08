use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ola_types::{Bytes, H256};

#[cfg_attr(
    all(feature = "client", feature = "server"),
    rpc(server, client, namespace = "ola")
)]
#[cfg_attr(
    all(feature = "client", not(feature = "server")),
    rpc(client, namespace = "ola")
)]
#[cfg_attr(
    all(not(feature = "client"), feature = "server"),
    rpc(server, namespace = "ola")
)]
pub trait OlaNamespace {
    #[method(name = "sendRawTransaction")]
    async fn send_raw_transaction(&self, tx_bytes: Bytes) -> RpcResult<H256>;
}