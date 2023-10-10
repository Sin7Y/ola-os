use std::time::Instant;

use ola_basic_types::H256;
use ola_web3_decl::error::Web3Error;
use web3::types::Bytes;

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

    pub async fn send_raw_transaction_impl(&self, tx_bytes: Bytes) -> Result<H256, Web3Error> {
        let start = Instant::now();
        let state = self.state.parse_transaction_bytes(&tx_bytes.0);
        Err(Web3Error::SerializationError(
            ola_types::request::SerializationTransactionError::InvalidPaymasterParams,
        ))
    }
}
