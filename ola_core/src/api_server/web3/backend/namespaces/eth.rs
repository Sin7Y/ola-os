use jsonrpsee::core::{async_trait, RpcResult};
use ola_types::api::{
    Block, BlockId, BlockNumber, Transaction, TransactionId, TransactionReceipt, TransactionVariant,
};
use ola_types::{api::BlockIdVariant, Address, H256, U256, U64};
use ola_web3_decl::namespaces::eth::EthNamespaceServer;
use web3::types::Index;

use crate::api_server::web3::{backend::into_rpc_error, namespaces::eth::EthNamespace};

#[async_trait]
impl EthNamespaceServer for EthNamespace {
    async fn get_block_number(&self) -> RpcResult<U64> {
        self.get_block_number_impl()
            .await
            .map_err(|err| self.current_method().map_err(err))
    }

    async fn chain_id(&self) -> RpcResult<U64> {
        Ok(self.chain_id_impl())
    }

    async fn gas_price(&self) -> RpcResult<U256> {
        todo!()
    }

    async fn get_balance(
        &self,
        address: Address,
        block: Option<BlockIdVariant>,
    ) -> RpcResult<U256> {
        todo!()
    }

    async fn get_block_by_number(
        &self,
        block_number: BlockNumber,
        full_transactions: bool,
    ) -> RpcResult<Option<Block<TransactionVariant>>> {
        self.get_block_impl(BlockId::Number(block_number), full_transactions)
            .await
            .map_err(|err| self.current_method().map_err(err))
    }

    async fn get_block_by_hash(
        &self,
        hash: H256,
        full_transactions: bool,
    ) -> RpcResult<Option<Block<TransactionVariant>>> {
        self.get_block_impl(BlockId::Hash(hash), full_transactions)
            .await
            .map_err(|err| self.current_method().map_err(err))
    }

    async fn get_block_transaction_count_by_number(
        &self,
        block_number: BlockNumber,
    ) -> RpcResult<Option<U256>> {
        self.get_block_transaction_count_impl(BlockId::Number(block_number))
            .await
            .map_err(|err| self.current_method().map_err(err))
    }

    async fn get_block_receipts(&self, block_id: BlockId) -> RpcResult<Vec<TransactionReceipt>> {
        self.get_block_receipts_impl(block_id)
            .await
            .map_err(|err| self.current_method().map_err(err))
    }

    async fn get_block_transaction_count_by_hash(
        &self,
        block_hash: H256,
    ) -> RpcResult<Option<U256>> {
        self.get_block_transaction_count_impl(BlockId::Hash(block_hash))
            .await
            .map_err(|err| self.current_method().map_err(err))
    }

    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockIdVariant>,
    ) -> RpcResult<u32> {
        self.get_transaction_count_impl(address, block.map(Into::into))
            .await
            .map_err(into_rpc_error)
    }

    async fn get_transaction_by_hash(&self, hash: H256) -> RpcResult<Option<Transaction>> {
        self.get_transaction_impl(TransactionId::Hash(hash))
            .await
            .map_err(|err| self.current_method().map_err(err))
    }

    async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: H256,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        self.get_transaction_impl(TransactionId::Block(BlockId::Hash(block_hash), index))
            .await
            .map_err(|err| self.current_method().map_err(err))
    }

    async fn get_transaction_by_block_number_and_index(
        &self,
        block_number: BlockNumber,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        self.get_transaction_impl(TransactionId::Block(BlockId::Number(block_number), index))
            .await
            .map_err(|err| self.current_method().map_err(err))
    }

    async fn get_transaction_receipt(&self, hash: H256) -> RpcResult<Option<TransactionReceipt>> {
        self.get_transaction_receipt_impl(hash)
            .await
            .map_err(|err| self.current_method().map_err(err))
    }

    async fn protocol_version(&self) -> RpcResult<String> {
        Ok(self.protocol_version())
    }

    async fn accounts(&self) -> RpcResult<Vec<Address>> {
        todo!()
    }

    async fn coinbase(&self) -> RpcResult<Address> {
        todo!()
    }
}
