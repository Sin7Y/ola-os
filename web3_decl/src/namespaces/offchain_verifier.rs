use jsonrpsee::{core::RpcResult, proc_macros::rpc};

#[cfg_attr(
    all(feature = "client", feature = "server"),
    rpc(server, client, namespace = "offchain_verifier")
)]
#[cfg_attr(
    all(feature = "client", not(feature = "server")),
    rpc(client, namespace = "offchain_verifier")
)]
#[cfg_attr(
    all(not(feature = "client"), feature = "server"),
    rpc(server, namespace = "offchain_verifier")
)]
pub trait OffchainVerifierNamespace {
    #[method(name = "sendRawTransaction1")]
    async fn send_raw_transaction(&self) -> RpcResult<()>;
}
