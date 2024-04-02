use ola_types::api::{Block, TransactionVariant};
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
    pub async fn get_block_number_impl(&self) -> Result<U64, Web3Error> {
        let mut storage = self
            .state
            .connection_pool
            .access_storage_tagged("api")
            .await;
        let block_number = storage
            .blocks_dal()
            .get_sealed_miniblock_number()
            .await
            .context("get_sealed_miniblock_number")?
            .ok_or(Web3Error::NoBlock)?;
        Ok(block_number.0.into())
    }

    // #[tracing::instrument(skip(self))]
    // pub async fn get_block_impl(
    //     &self,
    //     block_id: BlockId,
    //     full_transactions: bool,
    // ) -> Result<Option<Block<TransactionVariant>>, Web3Error> {
    //     self.current_method().set_block_id(block_id);
    //     self.state.start_info.ensure_not_pruned(block_id)?;
    //
    //     let block = self
    //         .state
    //         .connection_pool
    //         .access_storage_tagged("api")
    //         .await?
    //         .blocks_web3_dal()
    //         .get_block_by_web3_block_id(
    //             block_id,
    //             full_transactions,
    //             self.state.api_config.l2_chain_id,
    //         )
    //         .await
    //         .context("get_block_by_web3_block_id")?;
    //     if let Some(block) = &block {
    //         let block_number = MiniblockNumber(block.number.as_u32());
    //         self.set_block_diff(block_number);
    //     }
    //     Ok(block)
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub async fn get_block_transaction_count_impl(
    //     &self,
    //     block_id: BlockId,
    // ) -> Result<Option<U256>, Web3Error> {
    //     self.current_method().set_block_id(block_id);
    //     self.state.start_info.ensure_not_pruned(block_id)?;
    //
    //     let tx_count = self
    //         .state
    //         .connection_pool
    //         .access_storage_tagged("api")
    //         .await?
    //         .blocks_web3_dal()
    //         .get_block_tx_count(block_id)
    //         .await
    //         .context("get_block_tx_count")?;
    //
    //     if let Some((block_number, _)) = &tx_count {
    //         self.set_block_diff(*block_number);
    //     }
    //     Ok(tx_count.map(|(_, count)| count))
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub async fn get_block_receipts_impl(
    //     &self,
    //     block_id: BlockId,
    // ) -> Result<Vec<TransactionReceipt>, Web3Error> {
    //     self.current_method().set_block_id(block_id);
    //     self.state.start_info.ensure_not_pruned(block_id)?;
    //
    //     let block = self
    //         .state
    //         .connection_pool
    //         .access_storage_tagged("api")
    //         .await?
    //         .blocks_web3_dal()
    //         .get_block_by_web3_block_id(block_id, false, self.state.api_config.l2_chain_id)
    //         .await
    //         .context("get_block_by_web3_block_id")?;
    //     if let Some(block) = &block {
    //         self.set_block_diff(block.number.as_u32().into());
    //     }
    //
    //     let transactions: &[TransactionVariant] =
    //         block.as_ref().map_or(&[], |block| &block.transactions);
    //     let hashes: Vec<_> = transactions
    //         .iter()
    //         .map(|tx| match tx {
    //             TransactionVariant::Full(tx) => tx.hash,
    //             TransactionVariant::Hash(hash) => *hash,
    //         })
    //         .collect();
    //
    //     let mut receipts = self
    //         .state
    //         .connection_pool
    //         .access_storage_tagged("api")
    //         .await?
    //         .transactions_web3_dal()
    //         .get_transaction_receipts(&hashes)
    //         .await
    //         .context("get_transaction_receipts")?;
    //
    //     receipts.sort_unstable_by_key(|receipt| receipt.transaction_index);
    //     Ok(receipts)
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub async fn get_code_impl(
    //     &self,
    //     address: web3::types::Address,
    //     block_id: Option<BlockId>,
    // ) -> Result<Bytes, Web3Error> {
    //     let block_id = block_id.unwrap_or(BlockId::Number(BlockNumber::Pending));
    //     self.current_method().set_block_id(block_id);
    //
    //     let mut connection = self
    //         .state
    //         .connection_pool
    //         .access_storage_tagged("api")
    //         .await?;
    //     let block_number = self.state.resolve_block(&mut connection, block_id).await?;
    //     self.set_block_diff(block_number);
    //
    //     let contract_code = connection
    //         .storage_web3_dal()
    //         .get_contract_code_unchecked(address, block_number)
    //         .await
    //         .context("get_contract_code_unchecked")?;
    //     Ok(contract_code.unwrap_or_default().into())
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub fn chain_id_impl(&self) -> U64 {
    //     self.state.api_config.l2_chain_id.as_u64().into()
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub async fn get_storage_at_impl(
    //     &self,
    //     address: web3::types::Address,
    //     idx: U256,
    //     block_id: Option<BlockId>,
    // ) -> Result<H256, Web3Error> {
    //     let block_id = block_id.unwrap_or(BlockId::Number(BlockNumber::Pending));
    //     self.current_method().set_block_id(block_id);
    //
    //     let storage_key = StorageKey::new(AccountTreeId::new(address), u256_to_h256(idx));
    //     let mut connection = self
    //         .state
    //         .connection_pool
    //         .access_storage_tagged("api")
    //         .await?;
    //     let block_number = self.state.resolve_block(&mut connection, block_id).await?;
    //     self.set_block_diff(block_number);
    //     let value = connection
    //         .storage_web3_dal()
    //         .get_historical_value_unchecked(&storage_key, block_number)
    //         .await
    //         .context("get_historical_value_unchecked")?;
    //     Ok(value)
    // }
    //
    // /// Account nonce.
    // #[tracing::instrument(skip(self))]
    // pub async fn get_transaction_impl(
    //     &self,
    //     id: TransactionId,
    // ) -> Result<Option<Transaction>, Web3Error> {
    //     let mut transaction = self
    //         .state
    //         .connection_pool
    //         .access_storage_tagged("api")
    //         .await?
    //         .transactions_web3_dal()
    //         .get_transaction(id, self.state.api_config.l2_chain_id)
    //         .await
    //         .context("get_transaction")?;
    //
    //     if transaction.is_none() {
    //         transaction = self.state.tx_sink().lookup_tx(id).await?;
    //     }
    //     Ok(transaction)
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub async fn get_transaction_receipt_impl(
    //     &self,
    //     hash: H256,
    // ) -> Result<Option<TransactionReceipt>, Web3Error> {
    //     let receipts = self
    //         .state
    //         .connection_pool
    //         .access_storage_tagged("api")
    //         .await?
    //         .transactions_web3_dal()
    //         .get_transaction_receipts(&[hash])
    //         .await
    //         .context("get_transaction_receipts")?;
    //     Ok(receipts.into_iter().next())
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub async fn new_block_filter_impl(&self) -> Result<U256, Web3Error> {
    //     let installed_filters = self
    //         .state
    //         .installed_filters
    //         .as_ref()
    //         .ok_or(Web3Error::NotImplemented)?;
    //     let mut storage = self
    //         .state
    //         .connection_pool
    //         .access_storage_tagged("api")
    //         .await?;
    //     let last_block_number = storage
    //         .blocks_dal()
    //         .get_sealed_miniblock_number()
    //         .await
    //         .context("get_sealed_miniblock_number")?
    //         .context("no miniblocks in storage")?;
    //     let next_block_number = last_block_number + 1;
    //     drop(storage);
    //
    //     Ok(installed_filters
    //         .lock()
    //         .await
    //         .add(TypedFilter::Blocks(next_block_number)))
    // }
    //
    // #[tracing::instrument(skip(self, filter))]
    // pub async fn new_filter_impl(&self, mut filter: Filter) -> Result<U256, Web3Error> {
    //     let installed_filters = self
    //         .state
    //         .installed_filters
    //         .as_ref()
    //         .ok_or(Web3Error::NotImplemented)?;
    //     if let Some(topics) = filter.topics.as_ref() {
    //         if topics.len() > EVENT_TOPIC_NUMBER_LIMIT {
    //             return Err(Web3Error::TooManyTopics);
    //         }
    //     }
    //
    //     self.state.resolve_filter_block_hash(&mut filter).await?;
    //     let from_block = self.state.get_filter_from_block(&filter).await?;
    //     Ok(installed_filters
    //         .lock()
    //         .await
    //         .add(TypedFilter::Events(filter, from_block)))
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub async fn new_pending_transaction_filter_impl(&self) -> Result<U256, Web3Error> {
    //     let installed_filters = self
    //         .state
    //         .installed_filters
    //         .as_ref()
    //         .ok_or(Web3Error::NotImplemented)?;
    //     Ok(installed_filters
    //         .lock()
    //         .await
    //         .add(TypedFilter::PendingTransactions(
    //             chrono::Utc::now().naive_utc(),
    //         )))
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub async fn get_filter_changes_impl(&self, idx: U256) -> Result<FilterChanges, Web3Error> {
    //     let installed_filters = self
    //         .state
    //         .installed_filters
    //         .as_ref()
    //         .ok_or(Web3Error::NotImplemented)?;
    //     let mut filter = installed_filters
    //         .lock()
    //         .await
    //         .get_and_update_stats(idx)
    //         .ok_or(Web3Error::FilterNotFound)?;
    //
    //     match self.filter_changes(&mut filter).await {
    //         Ok(changes) => {
    //             installed_filters.lock().await.update(idx, filter);
    //             Ok(changes)
    //         }
    //         Err(Web3Error::LogsLimitExceeded(..)) => {
    //             // The filter was not being polled for a long time, so we remove it.
    //             installed_filters.lock().await.remove(idx);
    //             Err(Web3Error::FilterNotFound)
    //         }
    //         Err(err) => Err(err),
    //     }
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub async fn uninstall_filter_impl(&self, idx: U256) -> Result<bool, Web3Error> {
    //     let installed_filters = self
    //         .state
    //         .installed_filters
    //         .as_ref()
    //         .ok_or(Web3Error::NotImplemented)?;
    //     Ok(installed_filters.lock().await.remove(idx))
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub fn protocol_version(&self) -> String {
    //     // TODO (SMA-838): Versioning of our protocol
    //     PROTOCOL_VERSION.to_string()
    // }
    //
    // #[tracing::instrument(skip(self, tx_bytes))]
    // pub async fn send_raw_transaction_impl(&self, tx_bytes: Bytes) -> Result<H256, Web3Error> {
    //     let (mut tx, hash) = self.state.parse_transaction_bytes(&tx_bytes.0)?;
    //     tx.set_input(tx_bytes.0, hash);
    //
    //     let submit_result = self.state.tx_sender.submit_tx(tx).await;
    //     submit_result.map(|_| hash).map_err(|err| {
    //         tracing::debug!("Send raw transaction error: {err}");
    //         API_METRICS.submit_tx_error[&err.prom_error_code()].inc();
    //         err.into()
    //     })
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub fn accounts_impl(&self) -> Vec<web3::types::Address> {
    //     Vec::new()
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub fn syncing_impl(&self) -> SyncState {
    //     if let Some(state) = &self.state.sync_state {
    //         // Node supports syncing process (i.e. not the main node).
    //         if state.is_synced() {
    //             SyncState::NotSyncing
    //         } else {
    //             SyncState::Syncing(SyncInfo {
    //                 starting_block: 0u64.into(), // We always start syncing from genesis right now.
    //                 current_block: state.get_local_block().0.into(),
    //                 highest_block: state.get_main_node_block().0.into(),
    //             })
    //         }
    //     } else {
    //         // If there is no sync state, then the node is the main node and it's always synced.
    //         SyncState::NotSyncing
    //     }
    // }
    //
    // #[tracing::instrument(skip(self))]
    // pub async fn fee_history_impl(
    //     &self,
    //     block_count: U64,
    //     newest_block: BlockNumber,
    //     reward_percentiles: Vec<f32>,
    // ) -> Result<FeeHistory, Web3Error> {
    //     self.current_method()
    //         .set_block_id(BlockId::Number(newest_block));
    //
    //     // Limit `block_count`.
    //     let block_count = block_count
    //         .as_u64()
    //         .min(self.state.api_config.fee_history_limit)
    //         .max(1);
    //
    //     let mut connection = self
    //         .state
    //         .connection_pool
    //         .access_storage_tagged("api")
    //         .await?;
    //     let newest_miniblock = self
    //         .state
    //         .resolve_block(&mut connection, BlockId::Number(newest_block))
    //         .await?;
    //     self.set_block_diff(newest_miniblock);
    //
    //     let mut base_fee_per_gas = connection
    //         .blocks_web3_dal()
    //         .get_fee_history(newest_miniblock, block_count)
    //         .await
    //         .context("get_fee_history")?;
    //     // DAL method returns fees in DESC order while we need ASC.
    //     base_fee_per_gas.reverse();
    //
    //     let oldest_block = newest_miniblock.0 + 1 - base_fee_per_gas.len() as u32;
    //     // We do not store gas used ratio for blocks, returns array of zeroes as a placeholder.
    //     let gas_used_ratio = vec![0.0; base_fee_per_gas.len()];
    //     // Effective priority gas price is currently 0.
    //     let reward = Some(vec![
    //         vec![U256::zero(); reward_percentiles.len()];
    //         base_fee_per_gas.len()
    //     ]);
    //
    //     // `base_fee_per_gas` for next miniblock cannot be calculated, appending last fee as a placeholder.
    //     base_fee_per_gas.push(*base_fee_per_gas.last().unwrap());
    //     Ok(FeeHistory {
    //         oldest_block: web3::types::BlockNumber::Number(oldest_block.into()),
    //         base_fee_per_gas,
    //         gas_used_ratio,
    //         reward,
    //     })
    // }
    //
    // #[tracing::instrument(skip(self, typed_filter))]
    // async fn filter_changes(
    //     &self,
    //     typed_filter: &mut TypedFilter,
    // ) -> Result<FilterChanges, Web3Error> {
    //     Ok(match typed_filter {
    //         TypedFilter::Blocks(from_block) => {
    //             let mut conn = self
    //                 .state
    //                 .connection_pool
    //                 .access_storage_tagged("api")
    //                 .await?;
    //             let (block_hashes, last_block_number) = conn
    //                 .blocks_web3_dal()
    //                 .get_block_hashes_since(*from_block, self.state.api_config.req_entities_limit)
    //                 .await
    //                 .context("get_block_hashes_since")?;
    //
    //             *from_block = match last_block_number {
    //                 Some(last_block_number) => last_block_number + 1,
    //                 None => *from_block,
    //             };
    //
    //             FilterChanges::Hashes(block_hashes)
    //         }
    //
    //         TypedFilter::PendingTransactions(from_timestamp_excluded) => {
    //             let mut conn = self
    //                 .state
    //                 .connection_pool
    //                 .access_storage_tagged("api")
    //                 .await?;
    //             let (tx_hashes, last_timestamp) = conn
    //                 .transactions_web3_dal()
    //                 .get_pending_txs_hashes_after(
    //                     *from_timestamp_excluded,
    //                     Some(self.state.api_config.req_entities_limit),
    //                 )
    //                 .await
    //                 .context("get_pending_txs_hashes_after")?;
    //
    //             *from_timestamp_excluded = last_timestamp.unwrap_or(*from_timestamp_excluded);
    //
    //             FilterChanges::Hashes(tx_hashes)
    //         }
    //
    //         TypedFilter::Events(filter, from_block) => {
    //             let addresses = if let Some(addresses) = &filter.address {
    //                 addresses.0.clone()
    //             } else {
    //                 vec![]
    //             };
    //             let topics = if let Some(topics) = &filter.topics {
    //                 if topics.len() > EVENT_TOPIC_NUMBER_LIMIT {
    //                     return Err(Web3Error::TooManyTopics);
    //                 }
    //                 let topics_by_idx = topics.iter().enumerate().filter_map(|(idx, topics)| {
    //                     Some((idx as u32 + 1, topics.as_ref()?.0.clone()))
    //                 });
    //                 topics_by_idx.collect::<Vec<_>>()
    //             } else {
    //                 vec![]
    //             };
    //
    //             let mut to_block = self
    //                 .state
    //                 .resolve_filter_block_number(filter.to_block)
    //                 .await?;
    //
    //             if matches!(filter.to_block, Some(BlockNumber::Number(_))) {
    //                 to_block = to_block.min(
    //                     self.state
    //                         .resolve_filter_block_number(Some(BlockNumber::Latest))
    //                         .await?,
    //                 );
    //             }
    //
    //             let get_logs_filter = GetLogsFilter {
    //                 from_block: *from_block,
    //                 to_block,
    //                 addresses,
    //                 topics,
    //             };
    //
    //             let mut storage = self
    //                 .state
    //                 .connection_pool
    //                 .access_storage_tagged("api")
    //                 .await?;
    //
    //             // Check if there is more than one block in range and there are more than `req_entities_limit` logs that satisfies filter.
    //             // In this case we should return error and suggest requesting logs with smaller block range.
    //             if *from_block != to_block {
    //                 if let Some(miniblock_number) = storage
    //                     .events_web3_dal()
    //                     .get_log_block_number(
    //                         &get_logs_filter,
    //                         self.state.api_config.req_entities_limit,
    //                     )
    //                     .await
    //                     .context("get_log_block_number")?
    //                 {
    //                     return Err(Web3Error::LogsLimitExceeded(
    //                         self.state.api_config.req_entities_limit,
    //                         from_block.0,
    //                         miniblock_number.0 - 1,
    //                     ));
    //                 }
    //             }
    //
    //             let logs = storage
    //                 .events_web3_dal()
    //                 .get_logs(get_logs_filter, i32::MAX as usize)
    //                 .await
    //                 .context("get_logs")?;
    //             *from_block = to_block + 1;
    //             FilterChanges::Logs(logs)
    //         }
    //     })
    // }
}
