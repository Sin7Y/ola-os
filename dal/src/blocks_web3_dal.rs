use ola_types::{api, L1BatchNumber, MiniblockNumber, H256, U256, U64};
use sqlx::types::BigDecimal;
use sqlx::Row;

use crate::{
    models::storage_block::{bind_block_where_sql_params, web3_block_number_to_sql},
    SqlxError, StorageProcessor,
};

#[derive(Debug)]
pub struct BlocksWeb3Dal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl BlocksWeb3Dal<'_, '_> {
    pub async fn resolve_block_id(
        &mut self,
        block_id: api::BlockId,
    ) -> Result<Option<MiniblockNumber>, SqlxError> {
        let query_string = match block_id {
            api::BlockId::Hash(_) => "SELECT number FROM miniblocks WHERE hash = $1".to_owned(),
            api::BlockId::Number(api::BlockNumber::Number(_)) => {
                // The reason why instead of returning the `block_number` directly we use query is
                // to handle numbers of blocks that are not created yet.
                // the `SELECT number FROM miniblocks WHERE number=block_number` for
                // non-existing block number will returns zero.
                "SELECT number FROM miniblocks WHERE number = $1".to_owned()
            }
            api::BlockId::Number(api::BlockNumber::Earliest) => {
                return Ok(Some(MiniblockNumber(0)));
            }
            api::BlockId::Number(block_number) => web3_block_number_to_sql(block_number),
        };
        let row = bind_block_where_sql_params(&block_id, sqlx::query(&query_string))
            .fetch_optional(self.storage.conn())
            .await?;

        let block_number = row
            .and_then(|row| row.get::<Option<i64>, &str>("number"))
            .map(|n| MiniblockNumber(n as u32));
        Ok(block_number)
    }

    pub async fn get_block_details(
        &mut self,
        block_number: MiniblockNumber,
    ) -> sqlx::Result<Option<api::BlockDetails>> {
        let storage_block_details = sqlx::query_as!(
            StorageBlockDetails,
            r#"
            SELECT
                miniblocks.number,
                COALESCE(
                    miniblocks.l1_batch_number,
                    (
                        SELECT
                            (MAX(number) + 1)
                        FROM
                            l1_batches
                    )
                ) AS "l1_batch_number!",
                miniblocks.timestamp,
                miniblocks.l1_tx_count,
                miniblocks.l2_tx_count,
                miniblocks.hash AS "root_hash?",
                commit_tx.tx_hash AS "commit_tx_hash?",
                commit_tx.confirmed_at AS "committed_at?",
                prove_tx.tx_hash AS "prove_tx_hash?",
                prove_tx.confirmed_at AS "proven_at?",
                execute_tx.tx_hash AS "execute_tx_hash?",
                execute_tx.confirmed_at AS "executed_at?",
                miniblocks.l1_gas_price,
                miniblocks.l2_fair_gas_price,
                miniblocks.bootloader_code_hash,
                miniblocks.default_aa_code_hash,
                miniblocks.protocol_version,
                miniblocks.fee_account_address
            FROM
                miniblocks
                LEFT JOIN l1_batches ON miniblocks.l1_batch_number = l1_batches.number
                LEFT JOIN eth_txs_history AS commit_tx ON (
                    l1_batches.eth_commit_tx_id = commit_tx.eth_tx_id
                    AND commit_tx.confirmed_at IS NOT NULL
                )
                LEFT JOIN eth_txs_history AS prove_tx ON (
                    l1_batches.eth_prove_tx_id = prove_tx.eth_tx_id
                    AND prove_tx.confirmed_at IS NOT NULL
                )
                LEFT JOIN eth_txs_history AS execute_tx ON (
                    l1_batches.eth_execute_tx_id = execute_tx.eth_tx_id
                    AND execute_tx.confirmed_at IS NOT NULL
                )
            WHERE
                miniblocks.number = $1
            "#,
            block_number.0 as i64
        )
        .instrument("get_block_details")
        .with_arg("block_number", &block_number)
        .report_latency()
        .fetch_optional(self.storage)
        .await?;

        let Some(storage_block_details) = storage_block_details else {
            return Ok(None);
        };
        let mut details = api::BlockDetails::from(storage_block_details);

        // FIXME (PLA-728): remove after 2nd phase of `fee_account_address` migration
        #[allow(deprecated)]
        self.storage
            .blocks_dal()
            .maybe_load_fee_address(&mut details.operator_address, details.number)
            .await?;
        Ok(Some(details))
    }

    pub async fn get_l1_batch_details(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) -> sqlx::Result<Option<api::L1BatchDetails>> {
        let l1_batch_details: Option<StorageL1BatchDetails> = sqlx::query_as!(
            StorageL1BatchDetails,
            r#"
            WITH
                mb AS (
                    SELECT
                        l1_gas_price,
                        l2_fair_gas_price
                    FROM
                        miniblocks
                    WHERE
                        l1_batch_number = $1
                    LIMIT
                        1
                )
            SELECT
                l1_batches.number,
                l1_batches.timestamp,
                l1_batches.l1_tx_count,
                l1_batches.l2_tx_count,
                l1_batches.hash AS "root_hash?",
                commit_tx.tx_hash AS "commit_tx_hash?",
                commit_tx.confirmed_at AS "committed_at?",
                prove_tx.tx_hash AS "prove_tx_hash?",
                prove_tx.confirmed_at AS "proven_at?",
                execute_tx.tx_hash AS "execute_tx_hash?",
                execute_tx.confirmed_at AS "executed_at?",
                mb.l1_gas_price,
                mb.l2_fair_gas_price,
                l1_batches.bootloader_code_hash,
                l1_batches.default_aa_code_hash
            FROM
                l1_batches
                INNER JOIN mb ON TRUE
                LEFT JOIN eth_txs_history AS commit_tx ON (
                    l1_batches.eth_commit_tx_id = commit_tx.eth_tx_id
                    AND commit_tx.confirmed_at IS NOT NULL
                )
                LEFT JOIN eth_txs_history AS prove_tx ON (
                    l1_batches.eth_prove_tx_id = prove_tx.eth_tx_id
                    AND prove_tx.confirmed_at IS NOT NULL
                )
                LEFT JOIN eth_txs_history AS execute_tx ON (
                    l1_batches.eth_execute_tx_id = execute_tx.eth_tx_id
                    AND execute_tx.confirmed_at IS NOT NULL
                )
            WHERE
                l1_batches.number = $1
            "#,
            l1_batch_number.0 as i64
        )
        .instrument("get_l1_batch_details")
        .with_arg("l1_batch_number", &l1_batch_number)
        .report_latency()
        .fetch_optional(self.storage)
        .await?;

        Ok(l1_batch_details.map(Into::into))
    }
}
