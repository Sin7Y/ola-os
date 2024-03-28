use jsonrpsee::core::{async_trait, RpcResult};
use ola_types::api::proof_offchain_verification::{
    L1BatchDetailsWithOffchainVerification, OffChainVerificationResult,
};
use ola_types::api::{
    BlockDetails, BridgeAddresses, L1BatchDetails, L2ToL1LogProof, Proof, ProtocolVersion,
};
use ola_types::{
    api::{TransactionDetails, TransactionReceipt},
    request::CallRequest,
    Address, Bytes, L1BatchNumber, MiniblockNumber, Transaction, H256, U256, U64,
};
use ola_web3_decl::namespaces::ola::OlaNamespaceServer;
use ola_web3_decl::types::Token;
use std::collections::HashMap;

use crate::api_server::web3::{backend::into_rpc_error, namespaces::ola::OlaNamespace};

#[async_trait]
impl OlaNamespaceServer for OlaNamespace {
    async fn send_raw_transaction(&self, tx_bytes: Bytes) -> RpcResult<H256> {
        self.send_raw_transaction_impl(tx_bytes)
            .await
            .map_err(into_rpc_error)
    }

    async fn call_transaction(&self, call_request: CallRequest) -> RpcResult<Bytes> {
        self.call_impl(call_request).await.map_err(into_rpc_error)
    }

    async fn get_transaction_details(&self, hash: H256) -> RpcResult<Option<TransactionDetails>> {
        self.get_transaction_details_impl(hash)
            .await
            .map_err(into_rpc_error)
    }

    async fn get_transaction_receipt(&self, hash: H256) -> RpcResult<Option<TransactionReceipt>> {
        self.get_transaction_receipt_impl(hash)
            .await
            .map_err(into_rpc_error)
    }

    async fn l1_chain_id(&self) -> RpcResult<U64> {
        self.l1_chain_id_impl().await.map_err(into_rpc_error)
    }

    async fn get_confirmed_tokens(&self, from: u32, limit: u8) -> RpcResult<Vec<Token>> {
        todo!()
    }

    async fn get_all_account_balances(
        &self,
        address: Address,
    ) -> RpcResult<HashMap<Address, U256>> {
        todo!()
    }

    async fn get_l2_to_l1_msg_proof(
        &self,
        block: MiniblockNumber,
        sender: Address,
        msg: H256,
        l2_log_position: Option<usize>,
    ) -> RpcResult<Option<L2ToL1LogProof>> {
        todo!()
    }

    async fn get_l2_to_l1_log_proof(
        &self,
        tx_hash: H256,
        index: Option<usize>,
    ) -> RpcResult<Option<L2ToL1LogProof>> {
        todo!()
    }

    async fn get_l1_batch_number(&self) -> RpcResult<U64> {
        self.get_l1_batch_number_impl()
            .await
            .map_err(into_rpc_error)
    }

    async fn get_miniblock_range(&self, batch: L1BatchNumber) -> RpcResult<Option<(U64, U64)>> {
        self.get_miniblock_range_impl(batch)
            .await
            .map_err(into_rpc_error)
    }

    async fn get_block_details(
        &self,
        block_number: MiniblockNumber,
    ) -> RpcResult<Option<BlockDetails>> {
        self.get_block_details_impl(block_number)
            .await
            .map_err(into_rpc_error)
    }

    async fn get_raw_block_transactions(
        &self,
        block_number: MiniblockNumber,
    ) -> RpcResult<Vec<Transaction>> {
        todo!()
    }

    async fn get_l1_batch_details(
        &self,
        batch: L1BatchNumber,
    ) -> RpcResult<Option<L1BatchDetails>> {
        self.get_l1_batch_details_impl(batch)
            .await
            .map_err(into_rpc_error)
    }

    async fn get_protocol_version(
        &self,
        version_id: Option<u16>,
    ) -> RpcResult<Option<ProtocolVersion>> {
        self.get_protocol_version_impl(version_id)
            .await
            .map_err(into_rpc_error)
    }

    async fn get_l1_batch_details_with_offchain_verification(
        &self,
        batch: L1BatchNumber,
    ) -> RpcResult<Option<L1BatchDetailsWithOffchainVerification>> {
        todo!()
    }
}
