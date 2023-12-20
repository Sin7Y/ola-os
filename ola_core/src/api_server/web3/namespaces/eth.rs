use ola_types::{
    api::{BlockId, BlockNumber},
    Address, H256, U256,
};
use ola_web3_decl::error::Web3Error;
use web3::types::Bytes;

use crate::api_server::web3::{resolve_block, state::RpcState};

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

    #[tracing::instrument(skip(self, address, block_id))]
    pub async fn get_transaction_count_impl(
        &self,
        address: Address,
        block_id: Option<BlockId>,
    ) -> Result<u32, Web3Error> {
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

        let (full_nonce, block_number) = match block_id {
            BlockId::Number(BlockNumber::Pending) => {
                let nonce = connection
                    .transactions_web3_dal()
                    .next_nonce_by_initiator_account(address)
                    .await
                    .map_err(|err| Web3Error::InternalError);
                (nonce, None)
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
                    .map_err(|err| Web3Error::InternalError);
                (nonce, Some(block_number))
            }
        };

        let account_nonce = full_nonce.map(|nonce| nonce);
        account_nonce
    }
}
