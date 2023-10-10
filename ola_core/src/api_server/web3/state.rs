use ola_basic_types::{L1ChainId, L2ChainId};
use ola_config::{api::Web3JsonRpcConfig, sequencer::NetworkConfig};
use ola_types::api;
use ola_types::l2::L2Tx;
use ola_web3_decl::error::Web3Error;

#[derive(Debug, Clone)]
pub struct InternalApiconfig {
    pub l1_chain_id: L1ChainId,
    pub l2_chain_id: L2ChainId,
    pub max_tx_size: usize,
}

impl InternalApiconfig {
    pub fn new(eth_config: &NetworkConfig, web3_config: &Web3JsonRpcConfig) -> Self {
        Self {
            l1_chain_id: eth_config.network.chain_id(),
            l2_chain_id: L2ChainId(eth_config.ola_network_id),
            max_tx_size: web3_config.max_tx_size,
        }
    }
}

#[derive(Debug)]
pub struct RpcState {
    pub api_config: InternalApiconfig,
}

impl Clone for RpcState {
    fn clone(&self) -> Self {
        Self {
            api_config: self.api_config.clone(),
        }
    }
}

impl RpcState {
    pub fn parse_transaction_bytes(&self, bytes: &[u8]) -> Result<L2Tx, Web3Error> {
        let chain_id = self.api_config.l2_chain_id;
        let tx_request = api::TransactionRequest::from_bytes(bytes, chain_id.0)?;
        Ok(L2Tx::from_request(tx_request, self.api_config.max_tx_size)?)
    }
}
