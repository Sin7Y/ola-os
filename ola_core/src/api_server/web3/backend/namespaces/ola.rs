use jsonrpsee::{core::{async_trait, RpcResult}, types::ErrorObjectOwned};
use ola_basic_types::{Bytes, H256};
use ola_web3_decl::namespaces::ola::OlaNamespaceServer;

use crate::api_server::web3::{namespaces::ola::OlaNamespace, backend::into_rpc_error};

#[async_trait]
impl OlaNamespaceServer for OlaNamespace {
    async fn send_raw_transaction(&self, tx_bytes: Bytes) -> RpcResult<H256> {
        self.send_raw_transaction_impl(tx_bytes)
            .await
            .map_err(into_rpc_error)
    }
}