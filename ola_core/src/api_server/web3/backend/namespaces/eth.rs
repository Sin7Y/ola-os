use jsonrpsee::core::{async_trait, RpcResult};
use ola_types::{api::BlockIdVariant, Address};
use ola_web3_decl::namespaces::eth::EthNamespaceServer;

use crate::api_server::web3::{namespaces::eth::EthNamespace, backend::into_rpc_error};

#[async_trait]
impl EthNamespaceServer for EthNamespace {
    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockIdVariant>,
    ) -> RpcResult<u32> {
        self.get_transaction_count_impl(address, block.map(Into::into))
            .await
            .map_err(into_rpc_error)
    }
}
