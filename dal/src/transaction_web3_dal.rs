use ola_config::constants::contracts::{
    ACCOUNT_CODE_STORAGE_ADDRESS, FAILED_CONTRACT_DEPLOYMENT_BYTECODE_HASH,
};
use ola_types::{
    api::{self, BlockId, BlockNumber, TransactionDetails},
    Address, L2ChainId, MiniblockNumber, H2048, H256, U256, U64,
};
use ola_utils::h256_to_account_address;

use crate::models::storage_block::{bind_block_where_sql_params, web3_block_where_sql};
use crate::models::storage_transaction::{extract_web3_transaction, web3_transaction_select_sql};
use crate::{
    models::{
        storage_event::StorageWeb3Log,
        storage_transaction::{
            StorageTransaction, StorageTransactionDetails, StorageTransactionReceipt,
        },
    },
    SqlxError, StorageProcessor,
};

#[derive(Debug)]
pub struct TransactionsWeb3Dal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl TransactionsWeb3Dal<'_, '_> {
    pub async fn next_nonce_by_initiator_account(
        &mut self,
        initiator_address: Address,
    ) -> Result<u32, SqlxError> {
        let latest_block_number = self
            .storage
            .blocks_web3_dal()
            .resolve_block_id(BlockId::Number(BlockNumber::Latest))
            .await?
            .expect("Failed to get `latest` nonce");
        let latest_nonce = self
            .storage
            .storage_web3_dal()
            .get_address_historical_nonce(initiator_address, latest_block_number)
            .await?
            .as_u32();

        // Get nonces of non-rejected transactions, starting from the 'latest' nonce.
        // `latest` nonce is used, because it is guaranteed that there are no gaps before it.
        // `(miniblock_number IS NOT NULL OR error IS NULL)` is the condition that filters non-rejected transactions.
        // Query is fast because we have an index on (`initiator_address`, `nonce`)
        // and it cannot return more than `max_nonce_ahead` nonces.
        let non_rejected_nonces: Vec<u32> = sqlx::query!(
            r#"
            SELECT
                nonce AS "nonce!"
            FROM
                transactions
            WHERE
                initiator_address = $1
                AND nonce >= $2
                AND is_priority = FALSE
                AND (
                    miniblock_number IS NOT NULL
                    OR error IS NULL
                )
            ORDER BY
                nonce
            "#,
            initiator_address.as_bytes(),
            latest_nonce as i32
        )
        .fetch_all(self.storage.conn())
        .await?
        .into_iter()
        .map(|row| row.nonce as u32)
        .collect();

        // Find pending nonce as the first "gap" in nonces.
        let mut pending_nonce = latest_nonce;
        for nonce in non_rejected_nonces {
            if pending_nonce == nonce {
                pending_nonce += 1;
            } else {
                break;
            }
        }

        Ok(pending_nonce)
    }

    pub async fn get_transaction(
        &mut self,
        transaction_id: api::TransactionId,
        chain_id: L2ChainId,
    ) -> Result<Option<api::Transaction>, SqlxError> {
        let where_sql = match transaction_id {
            api::TransactionId::Hash(_) => "transactions.hash = $1".to_owned(),
            api::TransactionId::Block(block_id, _) => {
                format!(
                    "transactions.index_in_block = $1 AND {}",
                    web3_block_where_sql(block_id, 2)
                )
            }
        };
        let query = format!(
            "SELECT {}
            FROM transactions
            LEFT JOIN miniblocks ON miniblocks.number = transactions.miniblock_number
            WHERE {where_sql}",
            web3_transaction_select_sql()
        );
        let query = sqlx::query(&query);

        let query = match &transaction_id {
            api::TransactionId::Hash(tx_hash) => query.bind(tx_hash.as_bytes()),
            api::TransactionId::Block(block_id, tx_index) => {
                let tx_index = if tx_index.as_u64() > i32::MAX as u64 {
                    return Ok(None);
                } else {
                    tx_index.as_u64() as i32
                };
                bind_block_where_sql_params(block_id, query.bind(tx_index))
            }
        };

        let tx = query
            .fetch_optional(self.storage.conn())
            .await?
            .map(|row| extract_web3_transaction(row, chain_id));
        Ok(tx)
    }

    pub async fn get_transaction_receipt(
        &mut self,
        hash: H256,
    ) -> Result<Option<api::TransactionReceipt>, SqlxError> {
        {
            // TODO: check transactions.data->'to' as "transfer_to?",
            // and transactions.data->'contractAddress' as "execute_contract_address?",
            // TODO: is storage_log.key == contractAddress?
            let receipt = sqlx::query!(
                r#"
                WITH sl AS (
                    SELECT * FROM storage_logs
                    WHERE storage_logs.address = $1 AND storage_logs.tx_hash = $2
                    ORDER BY storage_logs.miniblock_number DESC, storage_logs.operation_number DESC
                    LIMIT 1
                )
                SELECT
                     transactions.hash as tx_hash,
                     transactions.index_in_block as index_in_block,
                     transactions.l1_batch_tx_index as l1_batch_tx_index,
                     transactions.miniblock_number as block_number,
                     transactions.error as error,
                     transactions.initiator_address as initiator_address,
                     transactions.data->'to' as "transfer_to?",
                     transactions.data->'contractAddress' as "execute_contract_address?",
                     transactions.tx_format as "tx_format?",
                     miniblocks.hash as "block_hash?",
                     miniblocks.l1_batch_number as "l1_batch_number?",
                     sl.key as "contract_address?"
                FROM transactions
                LEFT JOIN miniblocks
                    ON miniblocks.number = transactions.miniblock_number
                LEFT JOIN sl
                    ON sl.value != $3
                WHERE transactions.hash = $2
                "#,
                ACCOUNT_CODE_STORAGE_ADDRESS.as_bytes(),
                hash.as_bytes(),
                FAILED_CONTRACT_DEPLOYMENT_BYTECODE_HASH.as_bytes()
            )
            .fetch_optional(self.storage.conn())
            .await?
            .map(|db_row| {
                let status = match (db_row.block_number, db_row.error) {
                    (_, Some(_)) => Some(U64::from(0)),
                    (Some(_), None) => Some(U64::from(1)),
                    // tx not executed yet
                    _ => None,
                };
                let tx_type = db_row.tx_format.map(U64::from).unwrap_or_default();
                let transaction_index = db_row.index_in_block.map(U64::from).unwrap_or_default();

                let block_hash = db_row.block_hash.map(|bytes| H256::from_slice(&bytes));
                api::TransactionReceipt {
                    transaction_hash: H256::from_slice(&db_row.tx_hash),
                    transaction_index,
                    block_hash,
                    block_number: db_row.block_number.map(U64::from),
                    l1_batch_tx_index: db_row.l1_batch_tx_index.map(U64::from),
                    l1_batch_number: db_row.l1_batch_number.map(U64::from),
                    from: H256::from_slice(&db_row.initiator_address),
                    to: db_row
                        .transfer_to
                        .or(db_row.execute_contract_address)
                        .map(|addr| {
                            serde_json::from_value::<Address>(addr)
                                .expect("invalid address value in the database")
                        })
                        // For better compatibility with various clients, we never return null.
                        .or_else(|| Some(Address::default())),
                    gas_used: None,
                    effective_gas_price: None,
                    logs_bloom: H2048::default(),
                    cumulative_gas_used: U256::default(),
                    contract_address: db_row
                        .contract_address
                        .map(|addr| h256_to_account_address(&H256::from_slice(&addr))),
                    logs: vec![],
                    status,
                    root: block_hash,
                    // Even though the Rust SDK recommends us to supply "None" for legacy transactions
                    // we always supply some number anyway to have the same behaviour as most popular RPCs
                    transaction_type: Some(tx_type),
                }
            });
            match receipt {
                Some(mut receipt) => {
                    let logs: Vec<_> = sqlx::query_as!(
                        StorageWeb3Log,
                        r#"
                        SELECT
                            address, topic1, topic2, topic3, topic4, value,
                            Null::bytea as "block_hash", Null::bigint as "l1_batch_number?",
                            miniblock_number, tx_hash, tx_index_in_block,
                            event_index_in_block, event_index_in_tx
                        FROM events
                        WHERE tx_hash = $1
                        ORDER BY miniblock_number ASC, event_index_in_block ASC
                        "#,
                        hash.as_bytes()
                    )
                    .fetch_all(self.storage.conn())
                    .await?
                    .into_iter()
                    .map(|storage_log| {
                        let mut log = api::Log::from(storage_log);
                        log.block_hash = receipt.block_hash;
                        log.l1_batch_number = receipt.l1_batch_number;
                        log
                    })
                    .collect();

                    receipt.logs = logs;

                    Ok(Some(receipt))
                }
                None => Ok(None),
            }
        }
    }

    /// Returns receipts by transactions hashes.
    /// Hashes are expected to be unique.
    pub async fn get_transaction_receipts(
        &mut self,
        hashes: &[H256],
    ) -> Result<Vec<api::TransactionReceipt>, SqlxError> {
        let mut receipts: Vec<api::TransactionReceipt> = sqlx::query_as!(
            StorageTransactionReceipt,
            r#"
            WITH
                sl AS (
                    SELECT DISTINCT
                        ON (storage_logs.tx_hash) *
                    FROM
                        storage_logs
                    WHERE
                        storage_logs.address = $1
                        AND storage_logs.tx_hash = ANY ($3)
                    ORDER BY
                        storage_logs.tx_hash,
                        storage_logs.miniblock_number DESC,
                        storage_logs.operation_number DESC
                )
            SELECT
                transactions.hash AS tx_hash,
                transactions.index_in_block AS index_in_block,
                transactions.l1_batch_tx_index AS l1_batch_tx_index,
                transactions.miniblock_number AS "block_number!",
                transactions.error AS error,
                transactions.initiator_address AS initiator_address,
                transactions.data -> 'to' AS "transfer_to?",
                transactions.data -> 'contractAddress' AS "execute_contract_address?",
                transactions.tx_format AS "tx_format?",
                miniblocks.hash AS "block_hash",
                miniblocks.l1_batch_number AS "l1_batch_number?",
                sl.key AS "contract_address?"
            FROM
                transactions
                JOIN miniblocks ON miniblocks.number = transactions.miniblock_number
                LEFT JOIN sl ON sl.value != $2
                AND sl.tx_hash = transactions.hash
            WHERE
                transactions.hash = ANY ($3)
            "#,
            ACCOUNT_CODE_STORAGE_ADDRESS.as_bytes(),
            FAILED_CONTRACT_DEPLOYMENT_BYTECODE_HASH.as_bytes(),
            &hashes
                .iter()
                .map(|h| h.as_bytes().to_vec())
                .collect::<Vec<_>>()[..]
        )
        .fetch_all(self.storage.conn())
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

        let mut logs = self
            .storage
            .events_dal()
            .get_logs_by_tx_hashes(hashes)
            .await?;

        for receipt in &mut receipts {
            let logs_for_tx = logs.remove(&receipt.transaction_hash);

            if let Some(logs) = logs_for_tx {
                receipt.logs = logs
                    .into_iter()
                    .map(|mut log| {
                        log.block_hash = receipt.block_hash;
                        log.l1_batch_number = receipt.l1_batch_number;
                        log
                    })
                    .collect();
            }
        }

        Ok(receipts)
    }

    pub async fn get_transaction_details(
        &mut self,
        hash: H256,
    ) -> Result<Option<TransactionDetails>, SqlxError> {
        {
            let storage_tx_details: Option<StorageTransactionDetails> = sqlx::query_as!(
                StorageTransactionDetails,
                r#"
                    SELECT transactions.is_priority,
                        transactions.initiator_address,
                        transactions.received_at,
                        transactions.miniblock_number,
                        transactions.error
                    FROM transactions
                    LEFT JOIN miniblocks ON miniblocks.number = transactions.miniblock_number
                    LEFT JOIN l1_batches ON l1_batches.number = miniblocks.l1_batch_number
                    WHERE transactions.hash = $1
                "#,
                hash.as_bytes()
            )
            .fetch_optional(self.storage.conn())
            .await?;

            let tx = storage_tx_details.map(|tx_details| tx_details.into());

            Ok(tx)
        }
    }

    /// Returns the server transactions (not API ones) from a certain miniblock.
    /// Returns an empty list if the miniblock doesn't exist.
    pub async fn get_raw_miniblock_transactions(
        &mut self,
        miniblock: MiniblockNumber,
    ) -> sqlx::Result<Vec<ola_types::Transaction>> {
        let rows = sqlx::query_as!(
            StorageTransaction,
            r#"
            SELECT
                *
            FROM
                transactions
            WHERE
                miniblock_number = $1
            ORDER BY
                index_in_block
            "#,
            miniblock.0 as i64
        )
        .fetch_all(self.storage.conn())
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}
