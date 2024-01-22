use ola_types::H256;
use ola_types::{
    api::{BlockId, BlockNumber},
    l2::L2Tx,
    request::{CallRequest, TransactionRequest},
    Bytes,
};
use ola_web3_decl::error::Web3Error;

use crate::api_server::web3::state::RpcState;

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
        olaos_logs::info!("received a transaction: {:?}", tx_bytes);
        let (mut tx, hash) = self.state.parse_transaction_bytes(&tx_bytes.0)?;
        olaos_logs::debug!("parsed transaction: {:?}", tx);
        tx.set_input(tx_bytes.0, hash);

        let tx_chain_id = tx.common_data.extract_chain_id().unwrap_or_default();
        if self.state.api_config.l2_chain_id.0 != tx_chain_id {
            return Err(Web3Error::InvalidChainId(tx_chain_id));
        }

        let submit_result = self.state.tx_sender.submit_tx(tx).await;

        submit_result.map(|_| hash).map_err(|err| {
            olaos_logs::debug!("Send raw transaction error: {err}");
            Web3Error::SubmitTransactionError(err.to_string(), err.data())
        })
    }

    #[tracing::instrument(skip(self, request))]
    pub async fn call_impl(&self, request: CallRequest) -> Result<Bytes, Web3Error> {
        let tx = L2Tx::from_request(request.into(), self.state.api_config.max_tx_size)?;
        olaos_logs::debug!("parsed call request transaction: {:?}", tx);

        let call_result = self.state.tx_sender.call_transaction_impl(tx).await;
        let res_bytes = call_result.map_err(|err| {
            olaos_logs::debug!("Send raw transaction error: {err}");
            Web3Error::SubmitTransactionError(err.to_string(), err.data())
        })?;

        Ok(res_bytes.into())
    }
}
