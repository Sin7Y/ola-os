use crate::api_server::execution_sandbox::BlockStartInfo;
use anyhow::Context as _;
use ola_config::contracts::ContractsConfig;
use ola_config::{api::Web3JsonRpcConfig, sequencer::NetworkConfig};
use ola_dal::connection::ConnectionPool;
use ola_dal::StorageProcessor;
use ola_types::l2::L2Tx;
use ola_types::{api, L1BatchNumber, MiniblockNumber, U64};
use ola_types::{L1ChainId, L2ChainId, H256};
use ola_web3_decl::error::Web3Error;

use crate::api_server::tx_sender::TxSender;

#[derive(Debug, Clone)]
pub struct InternalApiConfig {
    pub l1_chain_id: L1ChainId,
    pub l2_chain_id: L2ChainId,
    pub max_tx_size: usize,
}

impl InternalApiConfig {
    pub fn new(
        eth_config: &NetworkConfig,
        web3_config: &Web3JsonRpcConfig,
        _contracts_config: &ContractsConfig,
    ) -> Self {
        Self {
            l1_chain_id: eth_config.network.chain_id(),
            l2_chain_id: L2ChainId(eth_config.ola_network_id),
            max_tx_size: web3_config.max_tx_size,
        }
    }
}

#[derive(Debug)]
pub(crate) enum PruneQuery {
    BlockId(api::BlockId),
    L1Batch(L1BatchNumber),
}

impl From<api::BlockId> for PruneQuery {
    fn from(id: api::BlockId) -> Self {
        Self::BlockId(id)
    }
}

impl From<MiniblockNumber> for PruneQuery {
    fn from(number: MiniblockNumber) -> Self {
        Self::BlockId(api::BlockId::Number(number.0.into()))
    }
}

impl From<L1BatchNumber> for PruneQuery {
    fn from(number: L1BatchNumber) -> Self {
        Self::L1Batch(number)
    }
}

#[derive(Debug, Clone)]
pub struct RpcState {
    pub api_config: InternalApiConfig,
    pub tx_sender: Option<TxSender>,
    pub connection_pool: ConnectionPool,
    pub start_info: BlockStartInfo,
}

impl RpcState {
    pub fn parse_transaction_bytes(&self, bytes: &[u8]) -> Result<(L2Tx, H256), Web3Error> {
        let chain_id = self.api_config.l2_chain_id;
        let (tx_request, hash) = api::TransactionRequest::from_bytes(bytes, chain_id.0)?;

        Ok((
            L2Tx::from_request(tx_request, self.api_config.max_tx_size)?,
            hash,
        ))
    }

    pub fn u64_to_block_number(n: U64) -> MiniblockNumber {
        if n.as_u64() > u32::MAX as u64 {
            MiniblockNumber(u32::MAX)
        } else {
            MiniblockNumber(n.as_u32())
        }
    }

    pub(crate) async fn resolve_block(
        &self,
        connection: &mut StorageProcessor<'_>,
        block: api::BlockId,
    ) -> Result<MiniblockNumber, Web3Error> {
        self.start_info.ensure_not_pruned(block)?;
        connection
            .blocks_web3_dal()
            .resolve_block_id(block)
            .await
            .context("resolve_block_id")?
            .ok_or(Web3Error::NoBlock)
    }

    pub async fn resolve_filter_block_number(
        &self,
        block_number: Option<api::BlockNumber>,
    ) -> Result<MiniblockNumber, Web3Error> {
        if let Some(api::BlockNumber::Number(number)) = block_number {
            return Ok(Self::u64_to_block_number(number));
        }

        let block_number = block_number.unwrap_or(api::BlockNumber::Latest);
        let block_id = api::BlockId::Number(block_number);
        let mut conn = self.connection_pool.access_storage_tagged("api").await;
        Ok(self.resolve_block(&mut conn, block_id).await.unwrap())
        // ^ `unwrap()` is safe: `resolve_block_id(api::BlockId::Number(_))` can only return `None`
        // if called with an explicit number, and we've handled this case earlier.
    }
}
