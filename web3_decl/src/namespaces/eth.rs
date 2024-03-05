use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    proc_macros::rpc,
};
use ola_types::{api::BlockIdVariant, Address};

use crate::types::{PubSubFilter, PubSubResult};

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
pub trait EthNamespace {
    #[method(name = "getTransactionCount")]
    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockIdVariant>,
    ) -> RpcResult<u32>;
}

#[rpc(server, namespace = "ola")]
pub trait EthPubSub {
    #[subscription(name = "subscribe" => "subscription", unsubscribe = "unsubscribe", item = PubSubResult)]
    async fn subscribe(&self, sub_type: String, filter: Option<PubSubFilter>)
        -> SubscriptionResult;
}
