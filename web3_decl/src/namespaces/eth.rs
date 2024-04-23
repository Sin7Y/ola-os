use crate::types::PubSubFilter;
use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    proc_macros::rpc,
};
use ola_types::{
    api::{
        Block, BlockId, BlockIdVariant, BlockNumber, Transaction, TransactionReceipt,
        TransactionVariant,
    },
    Address, Index, H256, U256, U64,
};

// use crate::types::{
//     Block, Bytes, FeeHistory, Filter, FilterChanges, Index, Log, PubSubFilter, SyncState,
//     TransactionReceipt, U256, U64,
// };

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
    #[method(name = "blockNumber")]
    async fn get_block_number(&self) -> RpcResult<U64>;

    #[method(name = "chainId")]
    async fn chain_id(&self) -> RpcResult<U64>;

    #[method(name = "getBlockByNumber")]
    async fn get_block_by_number(
        &self,
        block_number: BlockNumber,
        full_transactions: bool,
    ) -> RpcResult<Option<Block<TransactionVariant>>>;

    #[method(name = "getBlockByHash")]
    async fn get_block_by_hash(
        &self,
        hash: H256,
        full_transactions: bool,
    ) -> RpcResult<Option<Block<TransactionVariant>>>;

    #[method(name = "getBlockTransactionCountByNumber")]
    async fn get_block_transaction_count_by_number(
        &self,
        block_number: BlockNumber,
    ) -> RpcResult<Option<U256>>;

    #[method(name = "getBlockReceipts")]
    async fn get_block_receipts(&self, block_id: BlockId) -> RpcResult<Vec<TransactionReceipt>>;

    #[method(name = "getBlockTransactionCountByHash")]
    async fn get_block_transaction_count_by_hash(
        &self,
        block_hash: H256,
    ) -> RpcResult<Option<U256>>;

    #[method(name = "getTransactionCount")]
    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockIdVariant>,
    ) -> RpcResult<u32>;

    #[method(name = "getTransactionByHash")]
    async fn get_transaction_by_hash(&self, hash: H256) -> RpcResult<Option<Transaction>>;

    #[method(name = "getTransactionByBlockHashAndIndex")]
    async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        index: Index,
    ) -> RpcResult<Option<Transaction>>;

    #[method(name = "getTransactionByBlockNumberAndIndex")]
    async fn get_transaction_by_block_number_and_index(
        &self,
        block_number: BlockNumber,
        index: Index,
    ) -> RpcResult<Option<Transaction>>;

    #[method(name = "getTransactionReceipt")]
    async fn get_transaction_receipt(&self, hash: H256) -> RpcResult<Option<TransactionReceipt>>;

    #[method(name = "protocolVersion")]
    async fn protocol_version(&self) -> RpcResult<String>;
}

#[rpc(server, namespace = "ola")]
pub trait EthPubSub {
    #[subscription(name = "subscribe" => "subscription", unsubscribe = "unsubscribe", item = PubSubResult)]
    async fn subscribe(&self, sub_type: String, filter: Option<PubSubFilter>)
        -> SubscriptionResult;
}
