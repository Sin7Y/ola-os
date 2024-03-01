use ola_types::api::{TransactionDetails, TransactionReceipt};
use ola_types::H256;
use ola_types::{l2::L2Tx, request::CallRequest, Bytes};
use ola_web3_decl::error::Web3Error;

use crate::api_server::web3::backend::error::internal_error;
use crate::api_server::web3::state::RpcState;
use std::time::Instant;

#[derive(Debug)]
pub struct OlaNamespace {
    state: RpcState,
}

impl Clone for OlaNamespace {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl OlaNamespace {
    pub fn new(state: RpcState) -> Self {
        Self { state }
    }

    #[tracing::instrument(skip(self, tx_bytes))]
    pub async fn send_raw_transaction_impl(&self, tx_bytes: Bytes) -> Result<H256, Web3Error> {
        olaos_logs::info!("received a send transaction: {:?}", Instant::now());
        let (mut tx, hash) = self.state.parse_transaction_bytes(&tx_bytes.0)?;
        tx.set_input(tx_bytes.0, hash);
        olaos_logs::info!("parsed transaction, hash: {:?}, initiator_address: {:?}, contract address: {:?}, nonce: {:?}", tx.hash(), tx.initiator_account(), tx.recipient_account(), tx.nonce());

        let tx_chain_id = tx.common_data.extract_chain_id().unwrap_or_default();
        if self.state.api_config.l2_chain_id.0 != tx_chain_id {
            olaos_logs::info!("invalid chain id: {:?}", tx_chain_id);
            return Err(Web3Error::InvalidChainId(tx_chain_id));
        }

        let submit_result = self.state.tx_sender.as_ref().unwrap().submit_tx(tx).await;

        let res = submit_result.map(|_| hash).map_err(|err| {
            olaos_logs::info!("Send raw transaction error: {err}");
            Web3Error::SubmitTransactionError(err.to_string(), err.data())
        });

        olaos_logs::info!("Send raw transaction result: {:?}", res);

        res
    }

    #[tracing::instrument(skip(self, request))]
    pub async fn call_impl(&self, request: CallRequest) -> Result<Bytes, Web3Error> {
        olaos_logs::info!("received a call transaction request: {:?}", request);

        let tx = L2Tx::from_request(request.into(), self.state.api_config.max_tx_size)?;
        olaos_logs::info!("parsed call request transaction: {:?}", tx);

        let call_result = self
            .state
            .tx_sender
            .as_ref()
            .unwrap()
            .call_transaction_impl(tx)
            .await;
        let res_bytes = call_result.map_err(|err| {
            olaos_logs::info!("Send raw transaction error: {err}");
            Web3Error::SubmitTransactionError(err.to_string(), err.data())
        })?;

        olaos_logs::info!("Call transaction result: {:?}", res_bytes);

        Ok(res_bytes.into())
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_transaction_details_impl(
        &self,
        hash: H256,
    ) -> Result<Option<TransactionDetails>, Web3Error> {
        const METHOD_NAME: &str = "get_transaction_details";

        olaos_logs::info!("received a get transaction details, hash: {:?}", hash);

        let tx_details = self
            .state
            .connection_pool
            .access_storage_tagged("api")
            .await
            .transactions_web3_dal()
            .get_transaction_details(hash)
            .await
            .map_err(|err| internal_error(METHOD_NAME, err));

        olaos_logs::info!("api.web3.call get_transaction_details: {:?}", tx_details);

        tx_details
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_transaction_receipt_impl(
        &self,
        hash: H256,
    ) -> Result<Option<TransactionReceipt>, Web3Error> {
        const METHOD_NAME: &str = "get_transaction_receipt";

        let start = Instant::now();
        let receipt = self
            .state
            .connection_pool
            .access_storage_tagged("api")
            .await
            .transactions_web3_dal()
            .get_transaction_receipt(hash)
            .await
            .map_err(|err| internal_error(METHOD_NAME, err));

        olaos_logs::info!(
            "api.web3.call get_transaction_receipt: cost {:?}",
            start.elapsed()
        );
        receipt
    }
}
