use crate::types::Token;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use ola_types::{
    api::{
        proof_offchain_verification::{
            L1BatchDetailsWithOffchainVerification, OffChainVerificationResult,
        },
        BlockDetails, BridgeAddresses, L1BatchDetails, L2ToL1LogProof, Proof, ProtocolVersion,
        TransactionDetails, TransactionReceipt,
    },
    // fee::Fee,
    // fee_model::FeeParams,
    request::CallRequest,
    // transaction_request::CallRequest,
    Address,
    Bytes,
    L1BatchNumber,
    MiniblockNumber,
    H256,
    U256,
    U64,
};
use std::collections::HashMap;

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

    #[method(name = "callTransaction")]
    async fn call_transaction(&self, call_request: CallRequest) -> RpcResult<Bytes>;

    #[method(name = "getTransactionReceipt")]
    async fn get_transaction_receipt(&self, hash: H256) -> RpcResult<Option<TransactionReceipt>>;

    #[method(name = "postVerificationRes")]
    async fn post_verification_result(
        &self,
        verify_result: OffChainVerificationResult,
    ) -> RpcResult<bool>;

    #[method(name = "L1ChainId")]
    async fn l1_chain_id(&self) -> RpcResult<U64>;

    #[method(name = "getConfirmedTokens")]
    async fn get_confirmed_tokens(&self, from: u32, limit: u8) -> RpcResult<Vec<Token>>;

    #[method(name = "getAllAccountBalances")]
    async fn get_all_account_balances(&self, address: Address)
        -> RpcResult<HashMap<Address, U256>>;

    #[method(name = "getL2ToL1MsgProof")]
    async fn get_l2_to_l1_msg_proof(
        &self,
        block: MiniblockNumber,
        sender: Address,
        msg: H256,
        l2_log_position: Option<usize>,
    ) -> RpcResult<Option<L2ToL1LogProof>>;

    #[method(name = "getL2ToL1LogProof")]
    async fn get_l2_to_l1_log_proof(
        &self,
        tx_hash: H256,
        index: Option<usize>,
    ) -> RpcResult<Option<L2ToL1LogProof>>;

    #[method(name = "L1BatchNumber")]
    async fn get_l1_batch_number(&self) -> RpcResult<U64>;

    #[method(name = "getL1BatchBlockRange")]
    async fn get_miniblock_range(&self, batch: L1BatchNumber) -> RpcResult<Option<(U64, U64)>>;

    #[method(name = "getBlockDetails")]
    async fn get_block_details(
        &self,
        block_number: MiniblockNumber,
    ) -> RpcResult<Option<BlockDetails>>;

    #[method(name = "getTransactionDetails")]
    async fn get_transaction_details(&self, hash: H256) -> RpcResult<Option<TransactionDetails>>;

    #[method(name = "getRawBlockTransactions")]
    async fn get_raw_block_transactions(
        &self,
        block_number: MiniblockNumber,
    ) -> RpcResult<Vec<ola_types::Transaction>>;

    #[method(name = "getL1BatchDetails")]
    async fn get_l1_batch_details(&self, batch: L1BatchNumber)
        -> RpcResult<Option<L1BatchDetails>>;

    #[method(name = "getProtocolVersion")]
    async fn get_protocol_version(
        &self,
        version_id: Option<u16>,
    ) -> RpcResult<Option<ProtocolVersion>>;

    #[method(name = "getL1BatchDetailsWithOffchainVerification")]
    async fn get_l1_batch_details_with_offchain_verification(
        &self,
        batch: L1BatchNumber,
    ) -> RpcResult<Option<L1BatchDetailsWithOffchainVerification>>;
}
