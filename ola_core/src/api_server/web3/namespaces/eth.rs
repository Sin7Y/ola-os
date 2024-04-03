use anyhow::Context as _;
use ola_types::{
    api::{BlockId, BlockNumber},
    Address, MiniblockNumber, H256, U256, U64,
};
use ola_web3_decl::error::Web3Error;
use web3::types::{Bytes, FeeHistory, SyncInfo, SyncState};

use crate::api_server::web3::{backend::error::internal_error, resolve_block, state::RpcState};

#[derive(Debug)]
pub struct EthNamespace {
    state: RpcState,
}

impl Clone for EthNamespace {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl EthNamespace {
    pub fn new(state: RpcState) -> Self {
        Self { state }
    }

    #[olaos_logs::instrument(skip(self, address, block_id))]
    pub async fn get_transaction_count_impl(
        &self,
        address: Address,
        block_id: Option<BlockId>,
    ) -> anyhow::Result<u32, Web3Error> {
        let block_id = block_id.unwrap_or(BlockId::Number(BlockNumber::Pending));
        let method_name = match block_id {
            BlockId::Number(BlockNumber::Pending) => "get_pending_transaction_count",
            _ => "get_historical_transaction_count",
        };

        let mut connection = self
            .state
            .connection_pool
            .access_storage_tagged("api")
            .await;

        let full_nonce = match block_id {
            BlockId::Number(BlockNumber::Pending) => {
                let nonce = connection
                    .transactions_web3_dal()
                    .next_nonce_by_initiator_account(address)
                    .await
                    .map_err(|err| internal_error(method_name, err));
                nonce
            }
            _ => {
                let block_number = resolve_block(&mut connection, block_id, method_name).await?;
                let nonce = connection
                    .storage_web3_dal()
                    .get_address_historical_nonce(address, block_number)
                    .await
                    .map(|nonce_u256| {
                        let U256(ref arr) = nonce_u256;
                        arr[0] as u32
                    })
                    .map_err(|err| internal_error(method_name, err));
                nonce
            }
        };

        let account_nonce = full_nonce.map(|nonce| nonce);
        account_nonce
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_block_number_impl(&self) -> anyhow::Result<U64, Web3Error> {
        let mut storage = self
            .state
            .connection_pool
            .access_storage_tagged("api")
            .await;
        let block_number = storage.blocks_dal().get_sealed_miniblock_number().await;
        Ok(block_number.0.into())
    }
}
