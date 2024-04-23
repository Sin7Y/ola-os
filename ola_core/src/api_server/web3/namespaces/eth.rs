use crate::api_server::web3::{backend::error::internal_error, resolve_block, state::RpcState};
use anyhow::Context as _;
use ola_types::api::{Block, Transaction, TransactionId, TransactionReceipt, TransactionVariant};
use ola_types::{
    api::{BlockId, BlockNumber},
    Address, MiniblockNumber, H256, U256, U64,
};
use ola_web3_decl::error::Web3Error;
use web3::types::{Bytes, FeeHistory, SyncInfo, SyncState};

#[derive(Debug)]
pub struct EthNamespace {
    state: RpcState,
}

pub const PROTOCOL_VERSION: &str = "ola/1";

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

    #[tracing::instrument(skip(self))]
    pub fn protocol_version(&self) -> String {
        // TODO (SMA-838): Versioning of our protocol
        PROTOCOL_VERSION.to_string()
    }

    #[tracing::instrument(skip(self))]
    pub fn chain_id_impl(&self) -> U64 {
        self.state.api_config.l2_chain_id.0.into()
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_block_impl(
        &self,
        block_id: BlockId,
        full_transactions: bool,
    ) -> Result<Option<Block<TransactionVariant>>, Web3Error> {
        self.state.start_info.ensure_not_pruned(block_id)?;

        let block = self
            .state
            .connection_pool
            .access_storage_tagged("api")
            .await
            .blocks_web3_dal()
            .get_block_by_web3_block_id(
                block_id,
                full_transactions,
                self.state.api_config.l2_chain_id,
            )
            .await
            .map_err(|err| internal_error("get_block_by_web3_block_id", err))?;

        if let Some(block) = &block {
            let block_number = MiniblockNumber(block.number.as_u32());
        }
        Ok(block)
    }
    #[tracing::instrument(skip(self))]
    pub async fn get_block_transaction_count_impl(
        &self,
        block_id: BlockId,
    ) -> Result<Option<U256>, Web3Error> {
        self.state.start_info.ensure_not_pruned(block_id)?;

        let tx_count = self
            .state
            .connection_pool
            .access_storage_tagged("api")
            .await
            .blocks_web3_dal()
            .get_block_tx_count(block_id)
            .await
            .map_err(|err| internal_error("get_block_tx_count", err))?;

        Ok(tx_count.map(|(_, count)| count))
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_block_receipts_impl(
        &self,
        block_id: BlockId,
    ) -> Result<Vec<TransactionReceipt>, Web3Error> {
        self.state.start_info.ensure_not_pruned(block_id)?;

        let block = self
            .state
            .connection_pool
            .access_storage_tagged("api")
            .await
            .blocks_web3_dal()
            .get_block_by_web3_block_id(block_id, false, self.state.api_config.l2_chain_id)
            .await
            .map_err(|err| internal_error("get_block_by_web3_block_id", err))?;

        // if let Some(block) = &block {
        //     self.set_block_diff(block.number.as_u32().into());
        // }

        let transactions: &[TransactionVariant] =
            block.as_ref().map_or(&[], |block| &block.transactions);
        let hashes: Vec<_> = transactions
            .iter()
            .map(|tx| match tx {
                TransactionVariant::Full(tx) => tx.hash,
                TransactionVariant::Hash(hash) => *hash,
            })
            .collect();

        let mut receipts = self
            .state
            .connection_pool
            .access_storage_tagged("api")
            .await
            .transactions_web3_dal()
            .get_transaction_receipts(&hashes)
            .await
            .map_err(|err| internal_error("get_transaction_receipts", err))?;

        receipts.sort_unstable_by_key(|receipt| receipt.transaction_index);
        Ok(receipts)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_transaction_impl(
        &self,
        id: TransactionId,
    ) -> Result<Option<Transaction>, Web3Error> {
        let mut transaction = self
            .state
            .connection_pool
            .access_storage_tagged("api")
            .await
            .transactions_web3_dal()
            .get_transaction(id, self.state.api_config.l2_chain_id)
            .await
            .map_err(|err| internal_error("get_transaction", err))?;

        if transaction.is_none() {
            transaction = self.state.tx_sink().lookup_tx(id).await?;
        }
        Ok(transaction)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_transaction_receipt_impl(
        &self,
        hash: H256,
    ) -> Result<Option<TransactionReceipt>, Web3Error> {
        let receipts = self
            .state
            .connection_pool
            .access_storage_tagged("api")
            .await
            .transactions_web3_dal()
            .get_transaction_receipts(&[hash])
            .await
            .map_err(|err| internal_error("get_transaction_receipts", err))?;

        Ok(receipts.into_iter().next())
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
