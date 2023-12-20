use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ola_types::{api::BlockIdVariant, Address};

#[cfg_attr(
    all(feature = "client", feature = "server"),
    rpc(server, client, namespace = "eth")
)]
#[cfg_attr(
    all(feature = "client", not(feature = "server")),
    rpc(client, namespace = "eth")
)]
#[cfg_attr(
    all(not(feature = "client"), feature = "server"),
    rpc(server, namespace = "eth")
)]
pub trait EthNamespace {
    #[method(name = "getTransactionCount")]
    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockIdVariant>,
    ) -> RpcResult<u32>;
}
