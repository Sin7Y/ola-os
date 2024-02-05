use jsonrpsee::core::{async_trait, RpcResult};
use ola_types::{
    api::{BlockIdVariant, TransactionDetails},
    request::CallRequest,
    Bytes, H256,
};
use ola_web3_decl::namespaces::ola::OlaNamespaceServer;

use crate::api_server::web3::{backend::into_rpc_error, namespaces::ola::OlaNamespace};

#[async_trait]
impl OlaNamespaceServer for OlaNamespace {
    async fn send_raw_transaction(&self, tx_bytes: Bytes) -> RpcResult<H256> {
        self.send_raw_transaction_impl(tx_bytes)
            .await
            .map_err(into_rpc_error)
    }

    async fn call_transaction(&self, call_request: CallRequest) -> RpcResult<Bytes> {
        // TODO:
        let block: Option<BlockIdVariant> = None;
        self.call_impl(call_request, block.map(Into::into))
            .await
            .map_err(into_rpc_error)
    }

    async fn get_transaction_details(&self, hash: H256) -> RpcResult<Option<TransactionDetails>> {
        self.get_transaction_details_impl(hash)
            .await
            .map_err(into_rpc_error)
    }
}
